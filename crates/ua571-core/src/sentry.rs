//! Individual remote sentry unit state.

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::fire::FireTelemetry;
use crate::options::{OptionsState, WeaponStatus};

pub const SENTRY_COUNT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Sentry {
    /// 1-based display id (1..=4).
    pub id: u8,
    pub options: OptionsState,
    pub fire: FireTelemetry,
    pub link_ok: bool,
    pub online: bool,
}

impl Sentry {
    pub fn new(id: u8, starting_rounds: u16) -> Self {
        assert!((1..=SENTRY_COUNT as u8).contains(&id));
        Self {
            id,
            options: OptionsState::new(),
            fire: FireTelemetry::new(starting_rounds),
            link_ok: true,
            online: true,
        }
    }

    pub fn is_armed(&self) -> bool {
        self.options.weapon_status == WeaponStatus::Armed
    }

    pub fn label(&self) -> String {
        format!("SENTRY-{}", self.id)
    }
}

/// Fixed bank of four sentries (film setup).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SentryBank {
    units: [Sentry; SENTRY_COUNT],
}

impl SentryBank {
    pub fn new(starting_rounds: u16) -> Self {
        Self {
            units: std::array::from_fn(|i| Sentry::new((i + 1) as u8, starting_rounds)),
        }
    }

    pub fn get(&self, index: usize) -> Option<&Sentry> {
        self.units.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Sentry> {
        self.units.get_mut(index)
    }

    pub fn by_id(&self, id: u8) -> Option<&Sentry> {
        self.units.iter().find(|s| s.id == id)
    }

    pub fn by_id_mut(&mut self, id: u8) -> Option<&mut Sentry> {
        self.units.iter_mut().find(|s| s.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Sentry> {
        self.units.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Sentry> {
        self.units.iter_mut()
    }

    pub fn len(&self) -> usize {
        SENTRY_COUNT
    }

    pub fn is_empty(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_sentries() {
        let bank = SentryBank::new(500);
        assert_eq!(bank.len(), 4);
        assert_eq!(bank.get(0).unwrap().id, 1);
        assert_eq!(bank.get(3).unwrap().id, 4);
    }
}
