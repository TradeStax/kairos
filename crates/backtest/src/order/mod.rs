//! Order management for the backtest engine.
//!
//! This module provides the full order lifecycle: creating, modifying,
//! filling, cancelling, and expiring orders. It supports single orders,
//! bracket orders (entry + stop-loss + optional take-profit with OCO
//! linking), and bulk operations.
//!
//! # Submodules
//!
//! - [`book`] -- [`OrderBook`] that tracks all orders and their states.
//! - [`entity`] -- The [`Order`] struct representing a single order.
//! - [`request`] -- [`OrderRequest`] enum for strategy-to-engine
//!   communication.
//! - [`types`] -- Value types: [`OrderId`], [`OrderSide`],
//!   [`OrderType`], [`OrderStatus`], [`TimeInForce`].

pub mod book;
pub mod entity;
pub mod request;
pub mod types;

pub use book::OrderBook;
pub use entity::Order;
pub use request::{BracketOrder, NewOrder, OrderRequest};
pub use types::{OrderId, OrderSide, OrderStatus, OrderType, TimeInForce};
