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
    /// Space/Enter that skipped POST (or confirmed Options) must not fire
    /// until key-up. Unix crossterm delivers typematic as extra `Press` events.
    confirm_hold: ConfirmHold,
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
            confirm_hold: ConfirmHold::default(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut terminal = setup_terminal()?;
        let _restore = TerminalRestore;
        let tick_rate = Duration::from_millis(self.state.config.tick_ms);
        let mut last_tick = Instant::now();

        loop {
            terminal.draw(|f| views::draw(f, &self.state, &self.theme))?;

            let timeout = tick_rate.saturating_sub(last_tick.elapsed());
            if event::poll(timeout)? {
                match event::read()? {
                    Event::Key(key) => {
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
                break;
            }
        }

        Ok(())
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
            if key.kind != KeyEventKind::Release {
                self.state.quit();
            }
            return;
        }

        let is_confirm = matches!(key.code, KeyCode::Enter | KeyCode::Char(' '));
        let now = Instant::now();

        if is_confirm {
            match key.kind {
                KeyEventKind::Release => {
                    self.confirm_hold.release();
                    return;
                }
                KeyEventKind::Repeat => {
                    // Hold-to-fire only when the originating press was already on Fire.
                    if self.state.screen != Screen::Fire || self.confirm_hold.is_active() {
                        return;
                    }
                }
                KeyEventKind::Press => {
                    // Unix: typematic repeats look like Press. Ignore until release/gap.
                    if self.confirm_hold.is_held_press(now) {
                        return;
                    }
                }
            }
        } else if key.kind != KeyEventKind::Press {
            return;
        }

        if self.state.screen == Screen::Boot {
            // Any key skips boot.
            self.state.skip_boot();
            if is_confirm {
                self.confirm_hold.begin(now);
            }
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
                        // Remaining Press-repeats of this hold must not dump rounds.
                        self.confirm_hold.begin(now);
                    }
                    Screen::Boot => {}
                }
            }
            _ => {}
        }
    }
}

/// Gap after the last Space/Enter event beyond which Unix Press-repeats
/// (typematic delay is typically 250–1000 ms) are treated as a new press.
const CONFIRM_HOLD_GAP: Duration = Duration::from_millis(1000);

/// Latches Space/Enter after a non-Fire origin (POST skip / Options confirm)
/// until Release, or until events stop for [`CONFIRM_HOLD_GAP`] (Unix).
#[derive(Debug, Default)]
struct ConfirmHold {
    active: bool,
    last: Option<Instant>,
}

impl ConfirmHold {
    fn begin(&mut self, now: Instant) {
        self.active = true;
        self.last = Some(now);
    }

    fn release(&mut self) {
        self.active = false;
        self.last = None;
    }

    fn is_active(&self) -> bool {
        self.active
    }

    /// `true` when this Press is the same physical hold (ignore it).
    fn is_held_press(&mut self, now: Instant) -> bool {
        if !self.active {
            return false;
        }
        match self.last {
            Some(prev) if now.saturating_duration_since(prev) < CONFIRM_HOLD_GAP => {
                self.last = Some(now);
                true
            }
            _ => {
                self.release();
                false
            }
        }
    }
}

/// Restores cooked mode + primary screen on drop (errors, panic, or quit).
struct TerminalRestore;

impl Drop for TerminalRestore {
    fn drop(&mut self) {
        let _ = restore_terminal();
    }
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut out = stdout();
    if let Err(e) = execute!(out, EnterAlternateScreen) {
        let _ = disable_raw_mode();
        return Err(e.into());
    }
    let backend = CrosstermBackend::new(out);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ua571_core::Config;

    fn key(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)
    }

    /// Skip rodio/WASAPI: `FireAudio::try_new` ACCESS_VIOLATIONs under
    /// parallel `cargo test` on Windows CI.
    fn silent(config: Config) -> App {
        App {
            theme: ConsoleTheme::from_kind(config.theme),
            state: AppState::new(config),
            audio: None,
            confirm_hold: ConfirmHold::default(),
        }
    }

    fn boot_app() -> App {
        silent(Config {
            show_boot: true,
            demo_on_start: true,
            ..Config::default()
        })
    }

    fn options_app() -> App {
        silent(Config {
            show_boot: false,
            ..Config::default()
        })
    }

    #[test]
    fn confirm_hold_treats_close_presses_as_the_same_hold() {
        let mut hold = ConfirmHold::default();
        let t0 = Instant::now();
        hold.begin(t0);
        assert!(hold.is_active());
        assert!(hold.is_held_press(t0 + Duration::from_millis(30)));
        assert!(hold.is_held_press(t0 + Duration::from_millis(500)));
        hold.release();
        assert!(!hold.is_held_press(t0 + Duration::from_millis(510)));
    }

    #[test]
    fn confirm_hold_expires_after_typematic_gap() {
        let mut hold = ConfirmHold::default();
        let t0 = Instant::now();
        hold.begin(t0);
        assert!(hold.is_held_press(t0 + Duration::from_millis(20)));
        assert!(!hold.is_held_press(t0 + Duration::from_millis(20) + CONFIRM_HOLD_GAP));
        assert!(!hold.is_active());
    }

    #[test]
    fn hold_space_through_post_stays_on_options_with_demo() {
        let mut app = boot_app();
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Options);
        assert!(app.state.demo.is_active());

        // Unix typematic: extra Press events, not Repeat.
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Repeat));
        assert_eq!(app.state.screen, Screen::Options);
        assert!(app.state.demo.is_active());

        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Release));
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Fire);
        assert!(!app.state.demo.is_active());
    }

    #[test]
    fn hold_enter_through_post_does_not_confirm_options() {
        let mut app = boot_app();
        app.handle_key(key(KeyCode::Enter, KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Options);
        app.handle_key(key(KeyCode::Enter, KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Options);
        assert!(app.state.demo.is_active());
    }

    #[test]
    fn other_key_during_post_does_not_latch_space() {
        let mut app = boot_app();
        app.handle_key(key(KeyCode::Char('z'), KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Options);
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Fire);
    }

    #[test]
    fn space_on_options_does_not_hold_to_fire() {
        let mut app = options_app();
        app.handle_key(key(KeyCode::Char('a'), KeyEventKind::Press));
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.screen, Screen::Fire);
        let rounds = app.state.fire_telemetry().rounds;
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Repeat));
        assert_eq!(app.state.fire_telemetry().rounds, rounds);

        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Release));
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.fire_telemetry().rounds, rounds - 1);
    }

    #[test]
    fn space_repeat_on_fire_hold_to_fires() {
        let mut app = options_app();
        app.handle_key(key(KeyCode::Char('a'), KeyEventKind::Press));
        app.handle_key(key(KeyCode::Char('f'), KeyEventKind::Press));
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.fire_telemetry().rounds, 499);
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Repeat));
        assert_eq!(app.state.fire_telemetry().rounds, 498);
        app.handle_key(key(KeyCode::Char(' '), KeyEventKind::Press));
        assert_eq!(app.state.fire_telemetry().rounds, 497);
    }
}
