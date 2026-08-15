//! Terminal application event loop.

use std::io::{self, stdout};
use std::time::{Duration, Instant};

use color_eyre::eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use ua571_audio::FireAudio;
use ua571_core::{AppState, Config, Screen};

use crate::theme::ConsoleTheme;
use crate::views;

pub struct App {
    state: AppState,
    theme: ConsoleTheme,
    audio: Option<FireAudio>,
}

impl App {
    pub fn new(config: Config) -> Self {
        let theme = ConsoleTheme::from_kind(config.theme);
        let mut audio = FireAudio::try_new();
        if let Some(a) = audio.as_mut() {
            a.set_muted(!config.sound);
        }
        Self {
            state: AppState::new(config),
            theme,
            audio,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = setup_terminal()?;
        let tick_rate = Duration::from_millis(self.state.config.tick_ms);
        let mut last_tick = Instant::now();

        let result = loop {
            terminal.draw(|f| views::draw(f, &self.state, &self.theme))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.handle_key(key);
                    }
                    Event::Resize(_, _) => {}
                    _ => {}
                }
            }

            if last_tick.elapsed() >= tick_rate {
                self.state.tick();
                last_tick = Instant::now();
            }

            self.drain_sfx();

            if self.state.should_quit {
                break Ok(());
            }
        };

        restore_terminal()?;
        result
    }

    fn drain_sfx(&mut self) {
        let n = self.state.take_fire_sfx();
        if n > 0 {
            if let Some(audio) = self.audio.as_ref() {
                audio.play_fires(n);
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.state.quit();
            return;
        }

        if self.state.screen == Screen::Boot {
            // Any key skips boot.
            self.state.skip_boot();
            return;
        }

        // Demo: most keys stop auto-play so the operator can take over,
        // except pure navigation that we still allow.
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.state.quit();
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.state.toggle_demo();
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.state.toggle_sound();
                if let Some(audio) = self.audio.as_mut() {
                    audio.set_muted(!self.state.config.sound);
                }
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.theme = self.theme.next();
                self.state.config.theme = self.theme.kind;
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                self.state.stop_demo();
                self.state.set_screen(Screen::Fire);
            }
            KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Esc => {
                self.state.stop_demo();
                self.state.set_screen(Screen::Options);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.state.stop_demo();
                self.state.toggle_arm();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.state.stop_demo();
                self.state.reload();
            }
            KeyCode::Char('1') => {
                self.state.stop_demo();
                self.state.select_sentry(0);
            }
            KeyCode::Char('2') => {
                self.state.stop_demo();
                self.state.select_sentry(1);
            }
            KeyCode::Char('3') => {
                self.state.stop_demo();
                self.state.select_sentry(2);
            }
            KeyCode::Char('4') => {
                self.state.stop_demo();
                self.state.select_sentry(3);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.state.screen == Screen::Options {
                    self.state.stop_demo();
                    self.state.focus_prev_section();
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.state.screen == Screen::Options {
                    self.state.stop_demo();
                    self.state.focus_next_section();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.screen == Screen::Options {
                    self.state.stop_demo();
                    self.state.select_up();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.screen == Screen::Options {
                    self.state.stop_demo();
                    self.state.select_down();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.state.stop_demo();
                match self.state.screen {
                    Screen::Fire => {
                        let _ = self.state.fire();
                    }
                    Screen::Options => {
                        // Confirm → fire panel (original CONFIRM behavior).
                        self.state.set_screen(Screen::Fire);
                    }
                    Screen::Boot => {}
                }
            }
            _ => {}
        }
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}
