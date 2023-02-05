//! Inference server library.
//!
pub mod data_socket;
pub mod endpoints;
pub mod inferer;
pub mod nn;
pub mod pubsub;
pub mod utils;

/// Error type.
pub type Error = Box<dyn std::error::Error>;
