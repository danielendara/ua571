//! Configuration options matching the UA 571-C options panel sections.

use std::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Which options-panel section currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum MenuSection {
    #[default]
    SystemMode,
    WeaponStatus,
    IffStatus,
    TestRoutine,
    TargetProfile,
    SpectralProfile,
    TargetSelect,
}

impl MenuSection {
    pub const ALL: [MenuSection; 7] = [
        MenuSection::SystemMode,
        MenuSection::WeaponStatus,
        MenuSection::IffStatus,
        MenuSection::TestRoutine,
        MenuSection::TargetProfile,
        MenuSection::SpectralProfile,
        MenuSection::TargetSelect,
    ];

    pub fn index(self) -> usize {
        match self {
            MenuSection::SystemMode => 0,
            MenuSection::WeaponStatus => 1,
            MenuSection::IffStatus => 2,
            MenuSection::TestRoutine => 3,
            MenuSection::TargetProfile => 4,
            MenuSection::SpectralProfile => 5,
            MenuSection::TargetSelect => 6,
        }
    }

    pub fn from_index(i: usize) -> Option<Self> {
        Self::ALL.get(i).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            MenuSection::SystemMode => "SYSTEM MODE",
            MenuSection::WeaponStatus => "WEAPON STATUS",
            MenuSection::IffStatus => "IFF STATUS",
            MenuSection::TestRoutine => "TEST ROUTINE",
            MenuSection::TargetProfile => "TARGET PROFILE",
            MenuSection::SpectralProfile => "SPECTRAL PROFILE",
            MenuSection::TargetSelect => "TARGET SELECT",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            MenuSection::SystemMode => "SYSTEM",
            MenuSection::WeaponStatus => "WEAPON",
            MenuSection::IffStatus => "IFF",
            MenuSection::TestRoutine => "TEST",
            MenuSection::TargetProfile => "TARGET",
            MenuSection::SpectralProfile => "SPECTRAL",
            MenuSection::TargetSelect => "SELECT",
        }
    }

    pub fn next(self) -> Self {
        let i = (self.index() + 1) % Self::ALL.len();
        Self::ALL[i]
    }

    pub fn prev(self) -> Self {
        let i = if self.index() == 0 {
            Self::ALL.len() - 1
        } else {
            self.index() - 1
        };
        Self::ALL[i]
    }

    pub fn option_count(self) -> usize {
        match self {
            MenuSection::SystemMode => SystemMode::ALL.len(),
            MenuSection::WeaponStatus => WeaponStatus::ALL.len(),
            MenuSection::IffStatus => IffStatus::ALL.len(),
            MenuSection::TestRoutine => TestRoutine::ALL.len(),
            MenuSection::TargetProfile => TargetProfile::ALL.len(),
            MenuSection::SpectralProfile => SpectralProfile::ALL.len(),
            MenuSection::TargetSelect => TargetSelect::ALL.len(),
        }
    }
}

macro_rules! option_enum {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $variant:ident => $label:expr ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
        pub enum $name {
            #[default]
            $($variant),+
        }

        impl $name {
            pub const ALL: [$name; { 0 $(+ { let _ = $name::$variant; 1 })+ }] = [
                $($name::$variant),+
            ];

            pub fn label(self) -> &'static str {
                match self {
                    $($name::$variant => $label),+
                }
            }

            pub fn index(self) -> usize {
                Self::ALL.iter().position(|&v| v == self).unwrap_or(0)
            }

            pub fn from_index(i: usize) -> Option<Self> {
                Self::ALL.get(i).copied()
            }

            pub fn next(self) -> Self {
                let i = (self.index() + 1) % Self::ALL.len();
                Self::ALL[i]
            }

            pub fn prev(self) -> Self {
                let i = if self.index() == 0 {
                    Self::ALL.len() - 1
                } else {
                    self.index() - 1
                };
                Self::ALL[i]
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.label())
            }
        }
    };
}

option_enum! {
    SystemMode {
        AutoRemote => "AUTO-REMOTE",
        ManOverride => "MAN-OVERRIDE",
        SemiAuto => "SEMI-AUTO",
    }
}

option_enum! {
    WeaponStatus {
        Safe => "SAFE",
        Armed => "ARMED",
    }
}

option_enum! {
    IffStatus {
        Search => "SEARCH",
        Test => "TEST",
        Engaged => "ENGAGED",
        Interrogate => "INTERROGATE",
    }
}

option_enum! {
    TestRoutine {
        Auto => "AUTO",
        Selective => "SELECTIVE",
    }
}

option_enum! {
    TargetProfile {
        Soft => "SOFT",
        Semi => "SEMI",
        Hard => "HARD",
    }
}

option_enum! {
    SpectralProfile {
        Bio => "BIO",
        Inert => "INERT",
    }
}

option_enum! {
    TargetSelect {
        MultiSpec => "MULTI SPEC",
        InfraRed => "INFRA RED",
        UltraViolet => "UV",
    }
}

/// Full options configuration for one sentry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct OptionsState {
    pub focus: MenuSection,
    pub system_mode: SystemMode,
    pub weapon_status: WeaponStatus,
    pub iff_status: IffStatus,
    pub test_routine: TestRoutine,
    pub target_profile: TargetProfile,
    pub spectral_profile: SpectralProfile,
    pub target_select: TargetSelect,
}

impl OptionsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn focus_next_section(&mut self) {
        self.focus = self.focus.next();
    }

    pub fn focus_prev_section(&mut self) {
        self.focus = self.focus.prev();
    }

    /// Move selection down within the focused section.
    pub fn select_down(&mut self) {
        match self.focus {
            MenuSection::SystemMode => self.system_mode = self.system_mode.next(),
            MenuSection::WeaponStatus => self.weapon_status = self.weapon_status.next(),
            MenuSection::IffStatus => self.iff_status = self.iff_status.next(),
            MenuSection::TestRoutine => self.test_routine = self.test_routine.next(),
            MenuSection::TargetProfile => self.target_profile = self.target_profile.next(),
            MenuSection::SpectralProfile => self.spectral_profile = self.spectral_profile.next(),
            MenuSection::TargetSelect => self.target_select = self.target_select.next(),
        }
    }

    /// Move selection up within the focused section.
    pub fn select_up(&mut self) {
        match self.focus {
            MenuSection::SystemMode => self.system_mode = self.system_mode.prev(),
            MenuSection::WeaponStatus => self.weapon_status = self.weapon_status.prev(),
            MenuSection::IffStatus => self.iff_status = self.iff_status.prev(),
            MenuSection::TestRoutine => self.test_routine = self.test_routine.prev(),
            MenuSection::TargetProfile => self.target_profile = self.target_profile.prev(),
            MenuSection::SpectralProfile => self.spectral_profile = self.spectral_profile.prev(),
            MenuSection::TargetSelect => self.target_select = self.target_select.prev(),
        }
    }

    /// Index of the currently selected option in the focused section.
    pub fn focused_selection_index(&self) -> usize {
        match self.focus {
            MenuSection::SystemMode => self.system_mode.index(),
            MenuSection::WeaponStatus => self.weapon_status.index(),
            MenuSection::IffStatus => self.iff_status.index(),
            MenuSection::TestRoutine => self.test_routine.index(),
            MenuSection::TargetProfile => self.target_profile.index(),
            MenuSection::SpectralProfile => self.spectral_profile.index(),
            MenuSection::TargetSelect => self.target_select.index(),
        }
    }

    /// Labels for options in a given section, plus which is selected.
    pub fn section_options(self, section: MenuSection) -> (Vec<&'static str>, usize) {
        match section {
            MenuSection::SystemMode => (
                SystemMode::ALL.iter().map(|v| v.label()).collect(),
                self.system_mode.index(),
            ),
            MenuSection::WeaponStatus => (
                WeaponStatus::ALL.iter().map(|v| v.label()).collect(),
                self.weapon_status.index(),
            ),
            MenuSection::IffStatus => (
                IffStatus::ALL.iter().map(|v| v.label()).collect(),
                self.iff_status.index(),
            ),
            MenuSection::TestRoutine => (
                TestRoutine::ALL.iter().map(|v| v.label()).collect(),
                self.test_routine.index(),
            ),
            MenuSection::TargetProfile => (
                TargetProfile::ALL.iter().map(|v| v.label()).collect(),
                self.target_profile.index(),
            ),
            MenuSection::SpectralProfile => (
                SpectralProfile::ALL.iter().map(|v| v.label()).collect(),
                self.spectral_profile.index(),
            ),
            MenuSection::TargetSelect => (
                TargetSelect::ALL.iter().map(|v| v.label()).collect(),
                self.target_select.index(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_wraps() {
        let mut o = OptionsState::new();
        assert_eq!(o.focus, MenuSection::SystemMode);
        o.focus_prev_section();
        assert_eq!(o.focus, MenuSection::TargetSelect);
        o.focus_next_section();
        assert_eq!(o.focus, MenuSection::SystemMode);
    }

    #[test]
    fn selection_cycles() {
        let mut o = OptionsState::new();
        o.focus = MenuSection::WeaponStatus;
        assert_eq!(o.weapon_status, WeaponStatus::Safe);
        o.select_down();
        assert_eq!(o.weapon_status, WeaponStatus::Armed);
        o.select_down();
        assert_eq!(o.weapon_status, WeaponStatus::Safe);
    }
}
