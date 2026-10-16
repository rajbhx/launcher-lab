//! Hidden Space model.
//!
//! Mirrors the small state object REA recovered from the analyzed launcher
//! (`HiddenSpaceModel`): a boolean flag controls whether the space is gated
//! by a password, plus the list of hidden items.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HiddenItem {
    pub name: String,
    pub accent: u32,
}

#[derive(Clone, Debug)]
pub struct HiddenSpace {
    password_enabled: bool,
    unlocked: bool,
    items: Vec<HiddenItem>,
}

impl Default for HiddenSpace {
    fn default() -> Self {
        Self::new()
    }
}

impl HiddenSpace {
    pub fn new() -> Self {
        Self {
            password_enabled: false,
            unlocked: false,
            items: vec![
                HiddenItem {
                    name: "Vault".into(),
                    accent: 0x8A63F4,
                },
                HiddenItem {
                    name: "Diary".into(),
                    accent: 0x2F6FED,
                },
                HiddenItem {
                    name: "Wallet".into(),
                    accent: 0x1F9C6C,
                },
                HiddenItem {
                    name: "Backup".into(),
                    accent: 0xC77D2B,
                },
            ],
        }
    }

    pub fn password_enabled(&self) -> bool {
        self.password_enabled
    }

    /// Toggling the password on relocks the space; toggling it off leaves it
    /// open (matching the "flag off => direct access" behaviour).
    pub fn set_password_enabled(&mut self, on: bool) {
        self.password_enabled = on;
        self.unlocked = !on;
    }

    pub fn is_unlocked(&self) -> bool {
        !self.password_enabled || self.unlocked
    }

    pub fn unlock(&mut self) {
        self.unlocked = true;
    }

    pub fn lock(&mut self) {
        self.unlocked = false;
    }

    pub fn items(&self) -> &[HiddenItem] {
        &self.items
    }

    /// Add an item; returns `false` when an item with that name exists.
    pub fn hide_item(&mut self, name: &str, accent: u32) -> bool {
        if self.items.iter().any(|i| i.name == name) {
            return false;
        }
        self.items.push(HiddenItem {
            name: name.to_string(),
            accent,
        });
        true
    }

    pub fn unhide_item(&mut self, name: &str) -> Option<HiddenItem> {
        let idx = self.items.iter().position(|i| i.name == name)?;
        Some(self.items.remove(idx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_open_until_password_is_set() {
        let mut h = HiddenSpace::new();
        assert!(!h.password_enabled());
        assert!(h.is_unlocked());

        h.set_password_enabled(true);
        assert!(!h.is_unlocked());

        h.unlock();
        assert!(h.is_unlocked());

        h.set_password_enabled(false);
        assert!(h.is_unlocked());
    }

    #[test]
    fn relock_after_password_toggle() {
        let mut h = HiddenSpace::new();
        h.unlock();
        h.set_password_enabled(true);
        assert!(!h.is_unlocked());
        h.lock();
        assert!(!h.is_unlocked());
    }

    #[test]
    fn hide_and_unhide() {
        let mut h = HiddenSpace::new();
        let n = h.items().len();
        assert!(h.hide_item("Private", 0xFFAA00));
        assert!(!h.hide_item("Private", 0xFFAA00));
        assert_eq!(h.items().len(), n + 1);
        let removed = h.unhide_item("Private").expect("item present");
        assert_eq!(removed.name, "Private");
        assert!(h.unhide_item("Private").is_none());
    }
}
