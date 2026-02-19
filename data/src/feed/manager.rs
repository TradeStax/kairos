//! Data Feed Manager
//!
//! Manages the collection of configured data feeds with CRUD operations
//! and query methods.

use super::types::{DataFeed, FeedCapability, FeedId, FeedProvider, FeedStatus};
use serde::{Deserialize, Serialize};

/// Manages all configured data feeds
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DataFeedManager {
    feeds: Vec<DataFeed>,
}

impl DataFeedManager {
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================================================
    // CRUD Operations
    // ========================================================================

    /// Get all feeds
    pub fn feeds(&self) -> &[DataFeed] {
        &self.feeds
    }

    /// Get a feed by ID
    pub fn get(&self, id: FeedId) -> Option<&DataFeed> {
        self.feeds.iter().find(|f| f.id == id)
    }

    /// Get a mutable reference to a feed by ID
    pub fn get_mut(&mut self, id: FeedId) -> Option<&mut DataFeed> {
        self.feeds.iter_mut().find(|f| f.id == id)
    }

    /// Add a new feed. Returns false if a feed with the same ID already exists.
    pub fn add(&mut self, feed: DataFeed) -> bool {
        if self.feeds.iter().any(|f| f.id == feed.id) {
            log::warn!("Attempted to add feed with duplicate ID: {}", feed.id);
            return false;
        }
        self.feeds.push(feed);
        true
    }

    /// Remove a feed by ID. Returns the removed feed if found.
    pub fn remove(&mut self, id: FeedId) -> Option<DataFeed> {
        if let Some(pos) = self.feeds.iter().position(|f| f.id == id) {
            Some(self.feeds.remove(pos))
        } else {
            None
        }
    }

    /// Update a feed's status
    pub fn set_status(&mut self, id: FeedId, status: FeedStatus) {
        if let Some(feed) = self.get_mut(id) {
            feed.status = status;
        }
    }

    // ========================================================================
    // Query Operations
    // ========================================================================

    /// Get all enabled feeds
    pub fn enabled_feeds(&self) -> Vec<&DataFeed> {
        self.feeds.iter().filter(|f| f.enabled).collect()
    }

    /// Get feeds that support a given capability
    pub fn feeds_with_capability(&self, cap: FeedCapability) -> Vec<&DataFeed> {
        self.feeds
            .iter()
            .filter(|f| f.enabled && f.has_capability(cap))
            .collect()
    }

    /// Get the highest-priority feed for a capability (lowest priority number)
    pub fn primary_feed_for(&self, cap: FeedCapability) -> Option<&DataFeed> {
        self.feeds_with_capability(cap)
            .into_iter()
            .min_by_key(|f| f.priority)
    }

    /// Check if any enabled feed provides realtime data
    pub fn has_realtime(&self) -> bool {
        self.feeds.iter().any(|f| {
            f.enabled
                && f.capabilities()
                    .iter()
                    .any(|c| c.is_realtime())
        })
    }

    /// Count of currently connected feeds
    pub fn active_count(&self) -> usize {
        self.feeds
            .iter()
            .filter(|f| f.enabled && f.status.is_connected())
            .count()
    }

    /// Count of all feeds
    pub fn total_count(&self) -> usize {
        self.feeds.len()
    }

    /// Get feeds sorted by priority (lowest number first)
    pub fn feeds_by_priority(&self) -> Vec<&DataFeed> {
        let mut feeds: Vec<&DataFeed> = self.feeds.iter().collect();
        feeds.sort_by_key(|f| f.priority);
        feeds
    }

    /// Get feeds of a specific provider type
    pub fn feeds_for_provider(&self, provider: FeedProvider) -> Vec<&DataFeed> {
        self.feeds
            .iter()
            .filter(|f| f.provider == provider)
            .collect()
    }

    /// Get only historical dataset feeds
    pub fn historical_feeds(&self) -> Vec<&DataFeed> {
        self.feeds.iter().filter(|f| f.is_historical()).collect()
    }

    /// Get only realtime connection feeds
    pub fn realtime_feeds(&self) -> Vec<&DataFeed> {
        self.feeds.iter().filter(|f| f.is_realtime()).collect()
    }

    /// Check if any enabled feed of the given provider is currently connected
    pub fn has_connected_provider(&self, provider: FeedProvider) -> bool {
        self.feeds.iter().any(|f| {
            f.provider == provider && f.enabled && f.status.is_connected()
        })
    }

    /// Get the feed ID of a connected feed for the given provider (first match)
    pub fn connected_feed_id_for_provider(
        &self,
        provider: FeedProvider,
    ) -> Option<FeedId> {
        self.feeds
            .iter()
            .find(|f| {
                f.provider == provider && f.enabled && f.status.is_connected()
            })
            .map(|f| f.id)
    }

    // ========================================================================
    // Migration
    // ========================================================================

    /// Create a DataFeedManager from legacy configuration.
    ///
    /// If a Databento API key is present, creates a default Databento feed.
    pub fn migrate_from_legacy(has_databento_key: bool) -> Self {
        let mut manager = Self::new();

        if has_databento_key {
            let feed = DataFeed::new_databento("Databento");
            log::info!(
                "Migrated legacy config: created Databento feed '{}'",
                feed.id
            );
            manager.add(feed);
        }

        manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manager_crud() {
        let mut manager = DataFeedManager::new();
        assert_eq!(manager.total_count(), 0);

        // Add
        let feed = DataFeed::new_databento("Test");
        let id = feed.id;
        manager.add(feed);
        assert_eq!(manager.total_count(), 1);

        // Get
        assert!(manager.get(id).is_some());
        assert_eq!(manager.get(id).unwrap().name, "Test");

        // Get mut
        manager.get_mut(id).unwrap().name = "Updated".to_string();
        assert_eq!(manager.get(id).unwrap().name, "Updated");

        // Remove
        let removed = manager.remove(id);
        assert!(removed.is_some());
        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_manager_queries() {
        let mut manager = DataFeedManager::new();

        let mut databento = DataFeed::new_databento("Databento");
        databento.priority = 10;
        let databento_id = databento.id;

        let mut rithmic = DataFeed::new_rithmic("Rithmic");
        rithmic.priority = 5;

        manager.add(databento);
        manager.add(rithmic);

        // enabled_feeds
        assert_eq!(manager.enabled_feeds().len(), 2);

        // feeds_with_capability
        let historical = manager.feeds_with_capability(FeedCapability::HistoricalTrades);
        assert_eq!(historical.len(), 2);

        let realtime = manager.feeds_with_capability(FeedCapability::RealtimeTrades);
        assert_eq!(realtime.len(), 1);

        // primary_feed_for (Rithmic has lower priority number = higher priority)
        let primary = manager
            .primary_feed_for(FeedCapability::HistoricalTrades)
            .unwrap();
        assert_eq!(primary.provider, FeedProvider::Rithmic);

        // has_realtime
        assert!(manager.has_realtime());

        // active_count (none connected yet)
        assert_eq!(manager.active_count(), 0);

        // Set status
        manager.set_status(databento_id, FeedStatus::Connected);
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_manager_migration() {
        let manager = DataFeedManager::migrate_from_legacy(true);
        assert_eq!(manager.total_count(), 1);
        assert_eq!(
            manager.feeds()[0].provider,
            FeedProvider::Databento
        );

        let manager = DataFeedManager::migrate_from_legacy(false);
        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_manager_serialization() {
        let mut manager = DataFeedManager::new();
        manager.add(DataFeed::new_databento("Feed 1"));
        manager.add(DataFeed::new_rithmic("Feed 2"));

        let json = serde_json::to_string(&manager).unwrap();
        let deserialized: DataFeedManager = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_count(), 2);
    }

    #[test]
    fn test_feeds_by_priority() {
        let mut manager = DataFeedManager::new();

        let mut high = DataFeed::new_databento("High Priority");
        high.priority = 1;
        let mut low = DataFeed::new_rithmic("Low Priority");
        low.priority = 100;

        manager.add(low);
        manager.add(high);

        let sorted = manager.feeds_by_priority();
        assert_eq!(sorted[0].priority, 1);
        assert_eq!(sorted[1].priority, 100);
    }
}
