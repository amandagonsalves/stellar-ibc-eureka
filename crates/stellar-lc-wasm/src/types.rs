use prost::Message;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct Height {
    #[prost(uint64, tag = "1")]
    pub revision_number: u64,
    #[prost(uint64, tag = "2")]
    pub revision_height: u64,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct ScpEnvelope {
    #[prost(bytes = "vec", tag = "1")]
    pub node_id: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub statement_xdr: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub signature: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct ClientState {
    #[prost(string, tag = "1")]
    pub chain_id: alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub latest_height: ::core::option::Option<Height>,
    #[prost(message, optional, tag = "3")]
    pub frozen_height: ::core::option::Option<Height>,
    #[prost(bytes = "vec", repeated, tag = "4")]
    pub trusted_validators: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", repeated, tag = "5")]
    pub proof_specs: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    #[prost(bytes = "vec", tag = "6")]
    pub network_id: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct ConsensusState {
    #[prost(uint64, tag = "1")]
    pub timestamp: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub ledger_hash: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub root: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct StellarHeader {
    #[prost(uint64, tag = "1")]
    pub ledger_seq: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub ledger_header_xdr: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub ibc_state_root: alloc::vec::Vec<u8>,
    #[prost(message, repeated, tag = "4")]
    pub scp_envelopes: alloc::vec::Vec<ScpEnvelope>,
    #[prost(message, optional, tag = "5")]
    pub trusted_height: ::core::option::Option<Height>,
    #[prost(uint64, tag = "6")]
    pub timestamp: u64,
    #[prost(bytes = "vec", tag = "7")]
    pub ledger_hash: alloc::vec::Vec<u8>,
    #[prost(bytes = "vec", tag = "8")]
    pub previous_ledger_hash: alloc::vec::Vec<u8>,
}

#[derive(Clone, PartialEq, Eq, Message, Serialize, Deserialize)]
pub struct Misbehaviour {
    #[prost(string, tag = "1")]
    pub client_id: alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub header_1: ::core::option::Option<StellarHeader>,
    #[prost(message, optional, tag = "3")]
    pub header_2: ::core::option::Option<StellarHeader>,
}

extern crate alloc;
