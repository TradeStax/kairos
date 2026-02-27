//! History Plant connection pool.
//!
//! [`HistoryPlantPool`] manages a set of independently-connected history
//! plant instances enabling parallel day fetches. Each plant has its own
//! WebSocket, replay buffer, and authenticated login session.

use super::RithmicError;
use super::plants::{RithmicHistoryPlant, RithmicHistoryPlantHandle};
use super::protocol::RithmicResponse;
use super::protocol::config::RithmicConnectionConfig;
use super::protocol::ws::ConnectStrategy;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::Semaphore;

/// Default number of history plant connections in the pool.
///
/// Kept at 1 to minimize concurrent Rithmic sessions. Combined with the
/// ticker plant this gives 2 total WebSocket sessions, staying well within
/// Rithmic's per-user session limit and avoiding force-logout races.
const DEFAULT_POOL_SIZE: usize = 1;

/// A pool of Rithmic history plant connections for parallel
/// historical data fetching.
///
/// Each plant in the pool has its own WebSocket connection, replay
/// buffer, and authenticated session. The semaphore bounds
/// concurrency so callers can `acquire()` a handle and perform
/// independent paginated fetches in parallel.
pub struct HistoryPlantPool {
    /// Kept alive to maintain WebSocket connections
    _plants: Vec<RithmicHistoryPlant>,
    /// Cloneable handles for sending commands
    handles: Vec<RithmicHistoryPlantHandle>,
    /// Bounds concurrency to the number of connected plants
    semaphore: Arc<Semaphore>,
    /// Round-robin counter for handle selection
    next: AtomicUsize,
}

/// A borrowed handle from the pool.
///
/// The semaphore permit is held for the lifetime of this struct --
/// drop it to release the slot back to the pool.
pub struct PoolHandle {
    /// The underlying history plant handle for this slot
    handle: RithmicHistoryPlantHandle,
    /// Held to limit concurrency; released on drop
    _permit: tokio::sync::OwnedSemaphorePermit,
}

impl PoolHandle {
    /// Load historical ticks with automatic pagination.
    ///
    /// This is a standalone paginated fetch that uses this pool
    /// handle's dedicated history plant connection.
    pub async fn load_ticks(
        &self,
        symbol: &str,
        exchange: &str,
        start_secs: i32,
        end_secs: i32,
    ) -> Result<Vec<RithmicResponse>, RithmicError> {
        super::client::load_ticks_paginated(&self.handle, symbol, exchange, start_secs, end_secs)
            .await
    }
}

impl HistoryPlantPool {
    /// Connect a pool of history plants.
    ///
    /// Attempts to connect `pool_size` plants. Partial success is
    /// OK — if at least one plant connects, the pool is usable.
    /// If zero plants connect, returns the last connection error.
    pub async fn connect(
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
        pool_size: Option<usize>,
    ) -> Result<Self, RithmicError> {
        let target = pool_size.unwrap_or(DEFAULT_POOL_SIZE);
        let mut plants = Vec::with_capacity(target);
        let mut handles = Vec::with_capacity(target);
        let mut last_error = None;

        for i in 0..target {
            match Self::connect_one(config, strategy, i).await {
                Ok((plant, handle)) => {
                    plants.push(plant);
                    handles.push(handle);
                }
                Err(e) => {
                    log::warn!("History pool: plant {} failed to connect: {}", i, e,);
                    last_error = Some(e);
                }
            }
        }

        if handles.is_empty() {
            return Err(last_error.unwrap_or_else(|| {
                RithmicError::Connection("No history plants connected".to_string())
            }));
        }

        let connected = handles.len();
        log::info!("History pool: {}/{} plants connected", connected, target,);

        let semaphore = Arc::new(Semaphore::new(connected));

        Ok(Self {
            _plants: plants,
            handles,
            semaphore,
            next: AtomicUsize::new(0),
        })
    }

    /// Connect and authenticate a single history plant.
    async fn connect_one(
        config: &RithmicConnectionConfig,
        strategy: ConnectStrategy,
        index: usize,
    ) -> Result<(RithmicHistoryPlant, RithmicHistoryPlantHandle), RithmicError> {
        let plant = RithmicHistoryPlant::connect(config, strategy)
            .await
            .map_err(|e| {
                RithmicError::Connection(format!("History plant {} connect failed: {}", index, e,))
            })?;

        let handle = plant.get_handle();

        handle.login().await.map_err(|e| {
            RithmicError::Auth(format!("History plant {} login failed: {}", index, e,))
        })?;

        // Wait for any ForcedLogout + WebSocket Close frame to arrive and
        // kill the plant task. Observed close frames arrive ~500-600ms after
        // login; 1s provides comfortable margin.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Check if the plant task has exited (synchronous, non-racy).
        // When a ForcedLogout causes a Close frame, the plant's run loop
        // breaks and the JoinHandle completes.
        if plant.connection_handle.is_finished() {
            return Err(RithmicError::Connection(format!(
                "History plant {} died after login \
                 (likely force-logged out by server)",
                index,
            )));
        }

        log::info!("History pool: plant {} connected and authenticated", index);
        Ok((plant, handle))
    }

    /// Acquire a handle from the pool.
    ///
    /// Awaits until a permit is available (i.e., a plant is free),
    /// then returns a `PoolHandle` that auto-releases on drop.
    pub async fn acquire(&self) -> Result<PoolHandle, RithmicError> {
        let permit =
            self.semaphore.clone().acquire_owned().await.map_err(|_| {
                RithmicError::Connection("History pool semaphore closed".to_string())
            })?;

        // Round-robin handle selection
        let idx = self.next.fetch_add(1, Ordering::Relaxed) % self.handles.len();
        let handle = self.handles[idx].clone();

        Ok(PoolHandle {
            handle,
            _permit: permit,
        })
    }

    /// Returns the number of connected plants in the pool
    #[must_use]
    pub fn size(&self) -> usize {
        self.handles.len()
    }
}
