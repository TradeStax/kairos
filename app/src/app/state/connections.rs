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
}
