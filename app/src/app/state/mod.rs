//! Domain-grouped application state modules.
//!
//! Each struct owns a coherent subset of Kairos fields.

pub(crate) mod connections;
pub(crate) mod modals;
pub(crate) mod persistence;
pub(crate) mod services;
pub(crate) mod ui;

pub(crate) use connections::ConnectionState;
pub(crate) use modals::ModalState;
pub(crate) use persistence::PersistenceState;
pub(crate) use services::ServiceState;
pub(crate) use ui::UiState;
