pub mod data_socket;
pub mod endpoints;
pub mod inferer;
pub mod nn;
pub mod protocol;
pub mod pubsub;
pub mod utils;

type Error = Box<dyn std::error::Error>;
