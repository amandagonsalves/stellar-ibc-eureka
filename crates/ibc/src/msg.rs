use crate::error::Error;
use ibc::{core::handler::types::msgs::MsgEnvelope, primitives::proto::Any};
use prost::Message;

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum IbcMessage {
    Envelope(Box<MsgEnvelope>),
}

pub fn decode_message(bytes: &[u8]) -> Result<IbcMessage, Error> {
    if let Ok(any_msg) = Any::decode(bytes) {
        if let Ok(envelope) = MsgEnvelope::try_from(any_msg.clone()) {
            return Ok(IbcMessage::Envelope(Box::new(envelope)));
        }
    }

    Err(Error::Decoding(String::from("failed to decode tx data")))
}
