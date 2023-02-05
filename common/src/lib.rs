//! Common code shared between `infer_server` and `cam_sender`.
pub mod protocol;

/// Error type.
pub type Error = Box<dyn std::error::Error>;
