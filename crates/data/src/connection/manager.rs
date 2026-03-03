//! Connection manager — stores, queries, and resolves data feed connections.
//!
//! [`ConnectionManager`] owns the set of configured [`Connection`]s and
//! provides lookup, filtering, and priority-based resolution for chart loading.

use super::types::{Connection, ConnectionCapability, ConnectionProvider, ConnectionStatus};
use crate::domain::types::FeedId;
use serde::{Deserialize, Serialize};

/// Result of resolving the best connection for chart data loading.
#[must_use]
pub struct ResolvedConnection {
    /// Feed identifier for the resolved connection
    pub feed_id: FeedId,
    /// Data provider (Databento or Rithmic)
    pub provider: ConnectionProvider,
    /// Whether this connection supports historical data
    pub has_historical: bool,
}

/// Manages all configured data feed connections.
///
/// Provides CRUD operations, capability queries, and priority-based resolution
/// for selecting the best data source.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectionManager {
    connections: Vec<Connection>,
}

impl ConnectionManager {
    /// Creates an empty connection manager
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ── CRUD ───────────────────────────────────────────────────────────

    /// Returns all connections as a slice
    #[must_use]
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Returns all connections as a mutable slice
    pub fn connections_mut(&mut self) -> &mut [Connection] {
        &mut self.connections
    }

    /// Looks up a connection by feed ID
    #[must_use]
    pub fn get(&self, id: FeedId) -> Option<&Connection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// Returns a mutable reference to a connection by feed ID
    pub fn get_mut(&mut self, id: FeedId) -> Option<&mut Connection> {
        self.connections.iter_mut().find(|c| c.id == id)
    }

    /// Adds a connection. Returns `false` if a connection with the same ID already exists.
    pub fn add(&mut self, conn: Connection) -> bool {
        if self.connections.iter().any(|c| c.id == conn.id) {
            log::warn!("Attempted to add connection with duplicate ID: {}", conn.id);
            return false;
        }
        self.connections.push(conn);
        true
    }

    /// Removes and returns a connection by feed ID
    pub fn remove(&mut self, id: FeedId) -> Option<Connection> {
        if let Some(pos) = self.connections.iter().position(|c| c.id == id) {
            Some(self.connections.remove(pos))
        } else {
            None
        }
    }

    /// Updates the status of a connection by feed ID
    pub fn set_status(&mut self, id: FeedId, status: ConnectionStatus) {
        if let Some(conn) = self.get_mut(id) {
            conn.status = status;
        }
    }

    /// Returns the set of feed IDs for all connections NOT in `Disconnected` status.
    ///
    /// Used to filter stale data from async operations that complete after
    /// a feed has been explicitly disconnected by the user.
    #[must_use]
    pub fn active_feed_ids(&self) -> std::collections::HashSet<FeedId> {
        self.connections
            .iter()
            .filter(|c| !matches!(c.status, ConnectionStatus::Disconnected))
            .map(|c| c.id)
            .collect()
    }

    // ── Queries ────────────────────────────────────────────────────────

    /// Returns all enabled connections
    #[must_use]
    pub fn enabled_connections(&self) -> Vec<&Connection> {
        self.connections.iter().filter(|c| c.enabled).collect()
    }

    /// Returns all enabled connections that have a specific capability
    #[must_use]
    pub fn connections_with_capability(&self, cap: ConnectionCapability) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.enabled && c.has_capability(cap))
            .collect()
    }

    /// Returns the highest-priority enabled connection for a capability
    #[must_use]
    pub fn primary_for(&self, cap: ConnectionCapability) -> Option<&Connection> {
        self.connections_with_capability(cap)
            .into_iter()
            .min_by_key(|c| c.priority)
    }

    /// Returns `true` if any enabled connection supports real-time data
    #[must_use]
    pub fn has_realtime(&self) -> bool {
        self.connections
            .iter()
            .any(|c| c.enabled && c.capabilities().iter().any(|cap| cap.is_realtime()))
    }

    /// Returns the number of enabled and connected connections
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.connections
            .iter()
            .filter(|c| c.enabled && c.status.is_connected())
            .count()
    }

    /// Returns the total number of configured connections
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.connections.len()
    }

    /// Returns all connections sorted by priority (lowest first)
    #[must_use]
    pub fn connections_by_priority(&self) -> Vec<&Connection> {
        let mut conns: Vec<&Connection> = self.connections.iter().collect();
        conns.sort_by_key(|c| c.priority);
        conns
    }

    /// Returns all connections for a specific provider
    #[must_use]
    pub fn connections_for_provider(&self, provider: ConnectionProvider) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.provider == provider)
            .collect()
    }

    /// Returns all historical-mode connections
    #[must_use]
    pub fn historical_connections(&self) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.is_historical())
            .collect()
    }

    /// Returns all real-time-mode connections
    #[must_use]
    pub fn realtime_connections(&self) -> Vec<&Connection> {
        self.connections
            .iter()
            .filter(|c| c.is_realtime())
            .collect()
    }

    /// Returns `true` if there is an enabled, connected connection for the provider
    #[must_use]
    pub fn has_connected_provider(&self, provider: ConnectionProvider) -> bool {
        self.connections
            .iter()
            .any(|c| c.provider == provider && c.enabled && c.status.is_connected())
    }

    /// Returns the feed ID of the first enabled, connected connection for a provider
    #[must_use]
    pub fn connected_id_for_provider(&self, provider: ConnectionProvider) -> Option<FeedId> {
        self.connections
            .iter()
            .find(|c| c.provider == provider && c.enabled && c.status.is_connected())
            .map(|c| c.id)
    }

    /// Resolves the best available connection for chart data loading.
    ///
    /// Priority: Databento first (highest fidelity historical), then Rithmic.
    #[must_use]
    pub fn resolve_for_chart(&self) -> Option<ResolvedConnection> {
        if let Some(id) = self.connected_id_for_provider(ConnectionProvider::Databento) {
            return Some(ResolvedConnection {
                feed_id: id,
                provider: ConnectionProvider::Databento,
                has_historical: true,
            });
        }
        if let Some(id) = self.connected_id_for_provider(ConnectionProvider::Rithmic) {
            return Some(ResolvedConnection {
                feed_id: id,
                provider: ConnectionProvider::Rithmic,
                has_historical: true,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connection::Connection;

    #[test]
    fn test_manager_crud() {
        let mut manager = ConnectionManager::new();
        assert_eq!(manager.total_count(), 0);

        let conn = Connection::new_databento("Test");
        let id = conn.id;
        manager.add(conn);
        assert_eq!(manager.total_count(), 1);

        assert!(manager.get(id).is_some());
        assert_eq!(manager.get(id).unwrap().name, "Test");

        manager.get_mut(id).unwrap().name = "Updated".to_string();
        assert_eq!(manager.get(id).unwrap().name, "Updated");

        let removed = manager.remove(id);
        assert!(removed.is_some());
        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_manager_queries() {
        let mut manager = ConnectionManager::new();

        let mut databento = Connection::new_databento("Databento");
        databento.priority = 10;
        let databento_id = databento.id;

        let mut rithmic = Connection::new_rithmic("Rithmic");
        rithmic.priority = 5;

        manager.add(databento);
        manager.add(rithmic);

        assert_eq!(manager.enabled_connections().len(), 2);

        let historical =
            manager.connections_with_capability(ConnectionCapability::HistoricalTrades);
        assert_eq!(historical.len(), 2);

        let realtime = manager.connections_with_capability(ConnectionCapability::RealtimeTrades);
        assert_eq!(realtime.len(), 1);

        assert_eq!(manager.active_count(), 0);

        manager.set_status(databento_id, ConnectionStatus::Connected);
        assert_eq!(manager.active_count(), 1);
    }
}
