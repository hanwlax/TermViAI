//! Per-tab selective broadcast of already encoded terminal input.
//!
//! This package does not encode keys, own transports, or model pane layout.
//! The GUI adapter must refresh the pane snapshot from Mux before dispatch,
//! and must only pass user input (never terminal replies or mouse events).

mod input_router;
mod state;

pub use input_router::{route_input, DeliveryReport};
pub use state::{BroadcastState, UnknownPane};
