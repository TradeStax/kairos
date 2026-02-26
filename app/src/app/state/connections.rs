//! Live connection state: ConnectionManager.

pub(crate) struct ConnectionState {
    pub(crate) connection_manager: std::sync::Arc<std::sync::Mutex<data::ConnectionManager>>,
}

impl ConnectionState {
    pub(crate) fn new(
        connection_manager: std::sync::Arc<std::sync::Mutex<data::ConnectionManager>>,
    ) -> Self {
        Self { connection_manager }
    }

    /// Lock the ConnectionManager and call `f`.
    ///
    /// BORROW SAFETY: This method takes `&self`. If the caller needs to mutate other
    /// fields of the parent struct after calling this, the borrow must not overlap.
    pub(crate) fn with_connection_manager<R>(
        &self,
        f: impl FnOnce(&mut data::ConnectionManager) -> R,
    ) -> R {
        let mut guard = data::lock_or_recover(&self.connection_manager);
        f(&mut guard)
    }
}
