use std::collections::{HashMap, HashSet};

use uuid::Uuid;

/// Manages event subscriptions per-connection.
///
/// Clients subscribe to topic patterns using `events.subscribe` and
/// unsubscribe with `events.unsubscribe`. Events are only delivered
/// if they match at least one active subscription.
///
/// Matching rules:
/// - Exact match: `"terminal.session.exit"` matches only that topic.
/// - Prefix match with `*` wildcard: `"terminal.*"` matches any topic
///   starting with `"terminal."`.
/// - Bare `"*"` matches all topics.
#[derive(Debug, Default)]
pub struct SubscriptionManager {
    /// subscription_id → pattern
    subscriptions: HashMap<Uuid, String>,
    /// topic_prefix → set of subscription_ids (cached for fast lookup)
    prefix_index: HashMap<String, HashSet<Uuid>>,
    /// subscription_ids that are exact matches (no wildcard)
    exact_index: HashMap<String, HashSet<Uuid>>,
    /// subscription_ids that match everything ("*")
    catch_all: HashSet<Uuid>,
}

impl SubscriptionManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a subscription. Returns a subscription ID.
    pub fn subscribe(&mut self, pattern: impl Into<String>) -> Uuid {
        let pattern = pattern.into();
        let sub_id = Uuid::new_v4();

        if pattern == "*" {
            self.catch_all.insert(sub_id);
        } else if let Some(prefix) = pattern.strip_suffix(".*") {
            let prefix_with_dot = format!("{prefix}.");
            self.prefix_index
                .entry(prefix_with_dot)
                .or_default()
                .insert(sub_id);
        } else {
            self.exact_index
                .entry(pattern.clone())
                .or_default()
                .insert(sub_id);
        }

        self.subscriptions.insert(sub_id, pattern);
        sub_id
    }

    /// Remove a subscription by ID.
    pub fn unsubscribe(&mut self, sub_id: Uuid) -> bool {
        let pattern = match self.subscriptions.remove(&sub_id) {
            Some(p) => p,
            None => return false,
        };

        if pattern == "*" {
            self.catch_all.remove(&sub_id);
        } else if let Some(prefix) = pattern.strip_suffix(".*") {
            let prefix_with_dot = format!("{prefix}.");
            if let Some(set) = self.prefix_index.get_mut(&prefix_with_dot) {
                set.remove(&sub_id);
                if set.is_empty() {
                    self.prefix_index.remove(&prefix_with_dot);
                }
            }
        } else if let Some(set) = self.exact_index.get_mut(&pattern) {
            set.remove(&sub_id);
            if set.is_empty() {
                self.exact_index.remove(&pattern);
            }
        }

        true
    }

    /// Check if a topic matches any subscription.
    pub fn matches(&self, topic: &str) -> bool {
        // Catch-all
        if !self.catch_all.is_empty() {
            return true;
        }

        // Exact match
        if self
            .exact_index
            .get(topic)
            .is_some_and(|set| !set.is_empty())
        {
            return true;
        }

        // Prefix match: check all prefixes that could match
        for prefix in self.prefix_index.keys() {
            if topic.starts_with(prefix) {
                return true;
            }
        }

        false
    }

    /// Return true if there are no subscriptions.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }

    /// Number of active subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Remove all subscriptions.
    pub fn clear(&mut self) {
        self.subscriptions.clear();
        self.prefix_index.clear();
        self.exact_index.clear();
        self.catch_all.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("terminal.session.exit");

        assert!(mgr.matches("terminal.session.exit"));
        assert!(!mgr.matches("terminal.session.start"));
        assert!(!mgr.matches("agent.chat.delta"));
    }

    #[test]
    fn wildcard_prefix_match() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("terminal.*");

        assert!(mgr.matches("terminal.session.exit"));
        assert!(mgr.matches("terminal.session.start"));
        assert!(mgr.matches("terminal.anything"));
        assert!(!mgr.matches("agent.chat.delta"));
    }

    #[test]
    fn catch_all() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("*");

        assert!(mgr.matches("terminal.session.exit"));
        assert!(mgr.matches("agent.chat.delta"));
        assert!(mgr.matches("anything"));
    }

    #[test]
    fn unsubscribe_exact() {
        let mut mgr = SubscriptionManager::new();
        let id = mgr.subscribe("terminal.session.exit");

        assert!(mgr.matches("terminal.session.exit"));
        assert!(mgr.unsubscribe(id));
        assert!(!mgr.matches("terminal.session.exit"));
    }

    #[test]
    fn unsubscribe_wildcard() {
        let mut mgr = SubscriptionManager::new();
        let id = mgr.subscribe("terminal.*");

        assert!(mgr.matches("terminal.session.exit"));
        assert!(mgr.unsubscribe(id));
        assert!(!mgr.matches("terminal.session.exit"));
    }

    #[test]
    fn unsubscribe_catch_all() {
        let mut mgr = SubscriptionManager::new();
        let id = mgr.subscribe("*");

        assert!(mgr.matches("anything"));
        assert!(mgr.unsubscribe(id));
        assert!(!mgr.matches("anything"));
    }

    #[test]
    fn unsubscribe_nonexistent() {
        let mut mgr = SubscriptionManager::new();
        assert!(!mgr.unsubscribe(Uuid::new_v4()));
    }

    #[test]
    fn multiple_subscriptions() {
        let mut mgr = SubscriptionManager::new();
        let _id1 = mgr.subscribe("terminal.*");
        let id2 = mgr.subscribe("agent.*");

        assert!(mgr.matches("terminal.session.exit"));
        assert!(mgr.matches("agent.chat.delta"));
        assert!(!mgr.matches("files.list"));

        mgr.unsubscribe(id2);
        assert!(mgr.matches("terminal.session.exit"));
        assert!(!mgr.matches("agent.chat.delta"));
    }

    #[test]
    fn empty_after_clear() {
        let mut mgr = SubscriptionManager::new();
        mgr.subscribe("terminal.*");
        mgr.subscribe("*");

        assert!(!mgr.is_empty());
        mgr.clear();
        assert!(mgr.is_empty());
        assert!(!mgr.matches("terminal.session.exit"));
    }

    #[test]
    fn len_tracks_subscriptions() {
        let mut mgr = SubscriptionManager::new();
        assert_eq!(mgr.len(), 0);

        let id = mgr.subscribe("terminal.*");
        assert_eq!(mgr.len(), 1);

        mgr.subscribe("agent.*");
        assert_eq!(mgr.len(), 2);

        mgr.unsubscribe(id);
        assert_eq!(mgr.len(), 1);
    }
}
