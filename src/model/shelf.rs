//! Shelf card store, mirroring the binder contract REA recovered from the
//! OnePlus Launcher shelf provider (`CardProvider` / `CardContract`):
//!
//! * channels are registered by publishers (weather, health, music, memo, ...)
//! * cards live under a `(channel, tag)` key
//! * posting the same `(channel, tag)` pair updates the existing card
//! * cards can be fetched, deleted, or cleared per channel

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegisterChannelResult {
    Registered,
    AlreadyRegistered,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PostCardResult {
    Posted,
    Updated,
    ChannelNotFound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteCardResult {
    Deleted,
    NotFound,
}

#[derive(Clone, Debug)]
pub struct Card {
    pub id: u64,
    pub channel: String,
    pub tag: String,
    pub title: String,
    pub body: String,
    pub accent: u32,
    pub updated_seq: u64,
}

#[derive(Default)]
pub struct CardManager {
    /// channel id -> publisher token
    channels: BTreeMap<String, String>,
    /// (channel id, tag) -> card, matching the original `KEY_CHANNEL_ID` +
    /// `KEY_TAG` bundle keys.
    entries: BTreeMap<(String, String), Card>,
    next_id: u64,
    seq: u64,
}

impl CardManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_channel(&mut self, channel: &str, token: &str) -> RegisterChannelResult {
        if self.channels.contains_key(channel) {
            return RegisterChannelResult::AlreadyRegistered;
        }
        self.channels.insert(channel.to_string(), token.to_string());
        RegisterChannelResult::Registered
    }

    pub fn channel_registered(&self, channel: &str) -> bool {
        self.channels.contains_key(channel)
    }

    pub fn is_card_posted(&self, channel: &str, tag: &str) -> bool {
        self.entries.contains_key(&(channel.to_string(), tag.to_string()))
    }

    /// Post or update a card. `PostCardResult::Updated` is returned when a
    /// card already exists under the same `(channel, tag)` key.
    pub fn post_card(
        &mut self,
        channel: &str,
        tag: &str,
        title: &str,
        body: &str,
        accent: u32,
    ) -> PostCardResult {
        if !self.channels.contains_key(channel) {
            return PostCardResult::ChannelNotFound;
        }
        self.seq += 1;
        let key = (channel.to_string(), tag.to_string());
        let id = match self.entries.get(&key) {
            Some(prev) => prev.id,
            None => {
                self.next_id += 1;
                self.next_id
            }
        };
        let updated = self.entries.contains_key(&key);
        self.entries.insert(
            key,
            Card {
                id,
                channel: channel.to_string(),
                tag: tag.to_string(),
                title: title.to_string(),
                body: body.to_string(),
                accent,
                updated_seq: self.seq,
            },
        );
        if updated {
            PostCardResult::Updated
        } else {
            PostCardResult::Posted
        }
    }

    pub fn get_card(&self, channel: &str, tag: &str) -> Option<&Card> {
        self.entries.get(&(channel.to_string(), tag.to_string()))
    }

    pub fn delete_card(&mut self, channel: &str, tag: &str) -> DeleteCardResult {
        match self.entries.remove(&(channel.to_string(), tag.to_string())) {
            Some(_) => DeleteCardResult::Deleted,
            None => DeleteCardResult::NotFound,
        }
    }

    /// Remove every card of one channel; returns how many were removed.
    pub fn clear_channel(&mut self, channel: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(c, _), _| c != channel);
        before - self.entries.len()
    }

    /// All cards, most recently updated first.
    pub fn cards(&self) -> Vec<&Card> {
        let mut v: Vec<&Card> = self.entries.values().collect();
        v.sort_by(|a, b| b.updated_seq.cmp(&a.updated_seq));
        v
    }

    pub fn card_count(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_twice() {
        let mut m = CardManager::new();
        assert_eq!(
            m.register_channel("memo", "tok"),
            RegisterChannelResult::Registered
        );
        assert_eq!(
            m.register_channel("memo", "tok"),
            RegisterChannelResult::AlreadyRegistered
        );
        assert!(m.channel_registered("memo"));
        assert!(!m.channel_registered("weather"));
    }

    #[test]
    fn post_requires_registered_channel() {
        let mut m = CardManager::new();
        assert_eq!(
            m.post_card("memo", "t1", "A", "B", 0),
            PostCardResult::ChannelNotFound
        );
    }

    #[test]
    fn post_and_update_by_channel_and_tag() {
        let mut m = CardManager::new();
        m.register_channel("memo", "tok");
        assert_eq!(
            m.post_card("memo", "t1", "First", "body", 0),
            PostCardResult::Posted
        );
        assert_eq!(
            m.post_card("memo", "t1", "Second", "body2", 0),
            PostCardResult::Updated
        );
        assert_eq!(m.card_count(), 1);
        assert_eq!(m.get_card("memo", "t1").map(|c| c.title.as_str()), Some("Second"));
        assert!(m.is_card_posted("memo", "t1"));
        assert_eq!(
            m.post_card("memo", "t2", "Other", "", 0),
            PostCardResult::Posted
        );
        assert_eq!(m.card_count(), 2);
    }

    #[test]
    fn tags_are_scoped_per_channel() {
        let mut m = CardManager::new();
        m.register_channel("a", "t");
        m.register_channel("b", "t");
        m.post_card("a", "x", "A-x", "", 0);
        m.post_card("b", "x", "B-x", "", 0);
        assert_eq!(m.card_count(), 2);
        assert_eq!(m.get_card("a", "x").map(|c| c.title.as_str()), Some("A-x"));
        assert_eq!(m.get_card("b", "x").map(|c| c.title.as_str()), Some("B-x"));
    }

    #[test]
    fn delete_and_clear() {
        let mut m = CardManager::new();
        m.register_channel("memo", "tok");
        m.post_card("memo", "t1", "A", "", 0);
        m.post_card("memo", "t2", "B", "", 0);
        assert_eq!(m.delete_card("memo", "t1"), DeleteCardResult::Deleted);
        assert_eq!(m.delete_card("memo", "t1"), DeleteCardResult::NotFound);
        assert_eq!(m.clear_channel("memo"), 1);
        assert_eq!(m.card_count(), 0);
    }

    #[test]
    fn newest_first_ordering() {
        let mut m = CardManager::new();
        m.register_channel("n", "t");
        m.post_card("n", "old", "Old", "", 0);
        m.post_card("n", "new", "New", "", 0);
        m.post_card("n", "old", "Old v2", "", 0); // update moves it to the front
        let cards = m.cards();
        assert_eq!(cards[0].title, "Old v2");
        assert_eq!(cards[1].title, "New");
    }
}
