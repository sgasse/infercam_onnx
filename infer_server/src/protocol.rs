use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub enum ProtoMsg {
    FrameMsg(FrameMsg),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FrameMsg {
    pub id: String,
    pub data: Vec<u8>,
}

impl FrameMsg {
    pub fn new(id: String, data: Vec<u8>) -> Self {
        Self { id, data }
    }
}

#[cfg(test)]
mod test {

    use super::FrameMsg;

    #[test]
    fn test_bson_serde() {
        let frame_msg = FrameMsg {
            id: "bla".into(),
            data: vec![1, 2, 3],
        };

        let serialized: Vec<u8> = bincode::serialize(&frame_msg).unwrap();
        dbg!(&serialized);

        let deserialized_msg: FrameMsg = bincode::deserialize(&serialized[..]).unwrap();
        dbg!(deserialized_msg);
    }
}
