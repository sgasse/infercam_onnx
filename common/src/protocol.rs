//! Protocol definition for the data socket.
//!
use serde::{Deserialize, Serialize};

/// Definition of protocol messages.
#[derive(Debug, Deserialize, Serialize)]
pub enum ProtoMsg {
    ConnectReq(String),
    FrameMsg(FrameMsg),
}

/// Frame message.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FrameMsg {
    pub id: String,
    pub data: Vec<u8>,
}

impl FrameMsg {
    pub fn new(id: String, data: Vec<u8>) -> Self {
        Self { id, data }
    }
}

impl ProtoMsg {
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Box<bincode::ErrorKind>> {
        bincode::deserialize(bytes)
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::Error;

    #[test]
    fn test_bincode_serde() -> Result<(), Error> {
        let frame_msg = FrameMsg {
            id: "bla".into(),
            data: vec![1, 2, 3],
        };

        let serialized: Vec<u8> = bincode::serialize(&frame_msg)?;
        let deserialized_msg: FrameMsg = bincode::deserialize(&serialized[..])?;

        assert_eq!(frame_msg, deserialized_msg);

        Ok(())
    }
}
