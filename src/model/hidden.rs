//! Hidden Space model.
//!
//! Mirrors the small state object REA recovered from the analyzed launcher
//! (`HiddenSpaceModel`): a boolean flag controls whether the space is gated
//! by a password, plus the list of hidden items.
//!
//! On a device the primary rows are real installed apps ([`HiddenReal`],
//! hidden by holding them in the drawer); [`HiddenItem`] rows are the
//! reconstructed demo entries that only exist in the desktop preview.

use crate::icons::App;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HiddenItem {
    pub name: String,
    pub accent: u32,
}

/// A real, device-installed app that was hidden from the drawer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HiddenReal {
    pub package: String,
    pub activity: String,
    pub label: String,
}

/// One row of the Hidden Space listing, in display order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HiddenEntry {
    /// A demo-catalog app (desktop preview only).
    Mock(App),
    /// A real installed app hidden from the drawer.
    Real(HiddenReal),
    /// One of the reconstructed demo items.
    Demo(HiddenItem),
}

/// Display-order merge shared by rendering and input routing: the same
/// indices must be used on both sides, so both call this one function.
pub fn merge_hidden(mock: &[App], real: &[HiddenReal], demo: &[HiddenItem]) -> Vec<HiddenEntry> {
    let mut entries = Vec::with_capacity(mock.len() + real.len() + demo.len());
    entries.extend(real.iter().cloned().map(HiddenEntry::Real));
    entries.extend(mock.iter().copied().map(HiddenEntry::Mock));
    entries.extend(demo.iter().cloned().map(HiddenEntry::Demo));
    entries
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

    /// Remove every reconstructed demo item (a device build shows none).
    pub fn clear_items(&mut self) {
        self.items.clear();
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

    #[test]
    fn clear_items_empties_the_demo_list() {
        let mut h = HiddenSpace::new();
        assert!(!h.items().is_empty());
        h.clear_items();
        assert!(h.items().is_empty());
        assert!(h.is_unlocked());
    }

    #[test]
    fn merge_orders_real_then_mock_then_demo() {
        let mock = vec![App::Camera, App::Netflix];
        let real = vec![HiddenReal {
            package: "com.example.vault".to_owned(),
            activity: "com.example.vault.Main".to_owned(),
            label: "Vault".to_owned(),
        }];
        let demo = vec![HiddenItem {
            name: "Diary".to_owned(),
            accent: 0x123456,
        }];
        let entries = merge_hidden(&mock, &real, &demo);
        assert_eq!(entries.len(), 4);
        assert!(matches!(&entries[0], HiddenEntry::Real(r) if r.package == "com.example.vault"));
        assert!(matches!(&entries[1], HiddenEntry::Mock(App::Camera)));
        assert!(matches!(&entries[2], HiddenEntry::Mock(App::Netflix)));
        assert!(matches!(&entries[3], HiddenEntry::Demo(d) if d.name == "Diary"));
    }

    #[test]
    fn merge_indices_are_stable_and_empty_safe() {
        let entries = merge_hidden(&[], &[], &[]);
        assert!(entries.is_empty());
        let only_mock = merge_hidden(&[App::Chrome], &[], &[]);
        assert!(matches!(&only_mock[0], HiddenEntry::Mock(App::Chrome)));
    }
}
