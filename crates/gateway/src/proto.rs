use prost::Message;

#[derive(Clone, Message)]
pub struct LatestHeightRequest {}

#[derive(Clone, Message)]
pub struct LatestHeightResponse {
    #[prost(uint64, tag = "1")]
    pub revision_number: u64,
    #[prost(uint64, tag = "2")]
    pub revision_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryClientStateRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
    #[prost(uint64, tag = "2")]
    pub height: u64,
}

#[derive(Clone, Message)]
pub struct QueryClientStateResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub client_state: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryConsensusStateRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
    #[prost(uint64, tag = "2")]
    pub revision_number: u64,
    #[prost(uint64, tag = "3")]
    pub revision_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryConsensusStateResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub consensus_state: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryPacketCommitmentRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
    #[prost(uint64, tag = "2")]
    pub sequence: u64,
    #[prost(uint64, tag = "3")]
    pub height: u64,
}

#[derive(Clone, Message)]
pub struct QueryPacketCommitmentResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub commitment: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryPacketReceiptRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
    #[prost(uint64, tag = "2")]
    pub sequence: u64,
    #[prost(uint64, tag = "3")]
    pub height: u64,
}

#[derive(Clone, Message)]
pub struct QueryPacketReceiptResponse {
    #[prost(bool, tag = "1")]
    pub received: bool,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryNextSeqRecvRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
}

#[derive(Clone, Message)]
pub struct QueryNextSeqRecvResponse {
    #[prost(uint64, tag = "1")]
    pub next_seq_recv: u64,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
}

#[derive(Clone, Message)]
pub struct QueryIbcHeaderRequest {
    #[prost(uint64, tag = "1")]
    pub height: u64,
}

#[derive(Clone, Message)]
pub struct QueryIbcHeaderResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub header: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct SubmitSignedTxRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub tx_xdr: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct SubmitSignedTxResponse {
    #[prost(string, tag = "1")]
    pub tx_hash: String,
    #[prost(bytes = "vec", repeated, tag = "2")]
    pub events: Vec<Vec<u8>>,
}

#[derive(Clone, Message)]
pub struct MsgCreateClientRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub client_state: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub consensus_state: Vec<u8>,
    #[prost(string, tag = "3")]
    pub signer: String,
}

#[derive(Clone, Message)]
pub struct MsgCreateClientResponse {
    #[prost(string, tag = "1")]
    pub client_id: String,
}

#[derive(Clone, Message)]
pub struct MsgUpdateClientRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
    #[prost(bytes = "vec", tag = "2")]
    pub header: Vec<u8>,
    #[prost(string, tag = "3")]
    pub signer: String,
}

#[derive(Clone, Message)]
pub struct MsgUpdateClientResponse {}

#[derive(Clone, Message)]
pub struct MsgRegisterCounterpartyRequest {
    #[prost(string, tag = "1")]
    pub client_id: String,
    #[prost(string, tag = "2")]
    pub counterparty_client_id: String,
    #[prost(bytes = "vec", tag = "3")]
    pub merkle_prefix: Vec<u8>,
}

#[derive(Clone, Message)]
pub struct MsgRegisterCounterpartyResponse {}

#[derive(Clone, Message)]
pub struct MsgRecvPacketRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub packet: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
    #[prost(string, tag = "4")]
    pub signer: String,
}

#[derive(Clone, Message)]
pub struct MsgRecvPacketResponse {}

#[derive(Clone, Message)]
pub struct MsgAckPacketRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub packet: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub acknowledgement: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "4")]
    pub proof_height: u64,
    #[prost(string, tag = "5")]
    pub signer: String,
}

#[derive(Clone, Message)]
pub struct MsgAckPacketResponse {}

#[derive(Clone, Message)]
pub struct MsgTimeoutPacketRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub packet: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    pub proof: Vec<u8>,
    #[prost(uint64, tag = "3")]
    pub proof_height: u64,
    #[prost(string, tag = "4")]
    pub signer: String,
}

#[derive(Clone, Message)]
pub struct MsgTimeoutPacketResponse {}

#[derive(Clone, Message)]
pub struct StellarHeader {
    #[prost(uint32, tag = "1")]
    pub ledger_seq: u32,
    #[prost(bytes = "vec", tag = "2")]
    pub ledger_header_xdr: Vec<u8>,
    #[prost(bytes = "vec", tag = "3")]
    pub ibc_state_root: Vec<u8>,
    #[prost(bytes = "vec", tag = "4")]
    pub scp_node_id: Vec<u8>,
    #[prost(bytes = "vec", tag = "5")]
    pub scp_signature: Vec<u8>,
}

include!(concat!(
    env!("OUT_DIR"),
    "/stellar.gateway.v1.StellarGatewayQuery.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/stellar.gateway.v1.StellarGatewayMsg.rs"
));
