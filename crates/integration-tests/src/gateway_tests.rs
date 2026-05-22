use tonic::transport::{Channel, Endpoint};
use tonic::Code;

use crate::pb::{
    stellar_gateway_msg_client::StellarGatewayMsgClient,
    stellar_gateway_query_client::StellarGatewayQueryClient, LatestHeightRequest,
    MsgAckPacketRequest, MsgCreateClientRequest, MsgRecvPacketRequest,
    MsgRegisterCounterpartyRequest, MsgTimeoutPacketRequest, MsgUpdateClientRequest,
    QueryAcknowledgementRequest, QueryClientStateRequest, QueryConsensusStateRequest,
    QueryIbcHeaderRequest, QueryNextSeqRecvRequest, QueryPacketCommitmentRequest,
    QueryPacketReceiptRequest, SubmitSignedTxRequest,
};
use crate::{fail, pass};

const DEFAULT_GATEWAY_GRPC: &str = "http://0.0.0.0:50052";
const DEFAULT_CLIENT_ID: &str = "10-stellar-0";

pub fn gateway_addr() -> String {
    std::env::var("STELLAR_GATEWAY_GRPC_ADDR").unwrap_or_else(|_| DEFAULT_GATEWAY_GRPC.to_string())
}

async fn connect(addr: &str) -> Result<Channel, tonic::transport::Error> {
    Endpoint::from_shared(addr.to_string())?.connect().await
}

pub async fn run_all(addr: &str) {
    println!("\n--- gateway gRPC at {addr} ---");

    let channel = match connect(addr).await {
        Ok(c) => c,
        Err(e) => {
            fail("connect", format!("{e} (is the gateway running?)"));
            return;
        }
    };

    let height = test_latest_height(channel.clone()).await;

    let probe_height = height.unwrap_or(1);

    test_query_packet_commitment(channel.clone(), probe_height).await;
    test_query_packet_receipt(channel.clone(), probe_height).await;
    test_query_acknowledgement(channel.clone(), probe_height).await;
    test_query_client_state_unimplemented(channel.clone(), probe_height).await;
    test_query_consensus_state_unimplemented(channel.clone(), probe_height).await;
    test_query_next_seq_recv_unimplemented(channel.clone()).await;
    test_query_ibc_header(channel.clone(), probe_height).await;

    test_msg_submit_signed_tx_round_trip(channel.clone()).await;
    test_msg_create_client_round_trip(channel.clone()).await;
    test_msg_update_client_round_trip(channel.clone()).await;
    test_msg_register_counterparty_round_trip(channel.clone()).await;
    test_msg_recv_packet_round_trip(channel.clone()).await;
    test_msg_ack_packet_round_trip(channel.clone()).await;
    test_msg_timeout_packet_round_trip(channel).await;
}

async fn test_latest_height(channel: Channel) -> Option<u64> {
    let label = "LatestHeight: returns a non-zero revision_height";
    let mut client = StellarGatewayQueryClient::new(channel);
    match client.latest_height(LatestHeightRequest {}).await {
        Ok(resp) => {
            let h = resp.into_inner();
            if h.revision_height == 0 {
                fail(label, "revision_height is 0");
                return None;
            }
            if h.revision_number != 0 {
                fail(
                    label,
                    format!(
                        "expected revision_number=0 for Stellar, got {}",
                        h.revision_number
                    ),
                );
                return None;
            }
            pass(&format!("{label} (height: {})", h.revision_height));
            Some(h.revision_height)
        }
        Err(status) => {
            fail(label, format!("rpc failed: {status}"));
            None
        }
    }
}

async fn test_query_packet_commitment(channel: Channel, height: u64) {
    let label = "QueryPacketCommitment: returns non-membership proof for absent path";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_packet_commitment(QueryPacketCommitmentRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            sequence: 1,
            height,
        })
        .await;

    match resp {
        Ok(r) => {
            let body = r.into_inner();
            if !body.commitment.is_empty() {
                fail(
                    label,
                    "commitment unexpectedly non-empty for a key the SMT does not contain",
                );
                return;
            }
            if body.proof.is_empty() {
                fail(label, "proof is empty");
                return;
            }
            if body.proof_height != height {
                fail(
                    label,
                    format!(
                        "proof_height mismatch: requested {height}, got {}",
                        body.proof_height
                    ),
                );
                return;
            }
            pass(&format!("{label} (proof bytes: {})", body.proof.len()));
        }
        Err(status) => fail(label, format!("rpc failed: {status}")),
    }
}

async fn test_query_packet_receipt(channel: Channel, height: u64) {
    let label = "QueryPacketReceipt: reports received=false for absent path";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_packet_receipt(QueryPacketReceiptRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            sequence: 1,
            height,
        })
        .await;

    match resp {
        Ok(r) => {
            let body = r.into_inner();
            if body.received {
                fail(
                    label,
                    "received=true for a sequence the SMT does not contain",
                );
                return;
            }
            if body.proof.is_empty() {
                fail(label, "proof is empty");
                return;
            }
            pass(&format!("{label} (proof bytes: {})", body.proof.len()));
        }
        Err(status) => fail(label, format!("rpc failed: {status}")),
    }
}

async fn test_query_acknowledgement(channel: Channel, height: u64) {
    let label = "QueryAcknowledgement: returns non-membership proof for absent ack";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_acknowledgement(QueryAcknowledgementRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            sequence: 1,
            height,
        })
        .await;

    match resp {
        Ok(r) => {
            let body = r.into_inner();
            if !body.acknowledgement.is_empty() {
                fail(label, "acknowledgement unexpectedly non-empty");
                return;
            }
            if body.proof.is_empty() {
                fail(label, "proof is empty");
                return;
            }
            pass(&format!("{label} (proof bytes: {})", body.proof.len()));
        }
        Err(status) => fail(label, format!("rpc failed: {status}")),
    }
}

async fn test_query_client_state_unimplemented(channel: Channel, height: u64) {
    let label = "QueryClientState: returns Unimplemented (path not provable in v2)";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_client_state(QueryClientStateRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            height,
        })
        .await;

    match resp {
        Err(status) if status.code() == Code::Unimplemented => pass(label),
        Err(status) => fail(label, format!("expected Unimplemented, got {status}")),
        Ok(_) => fail(label, "expected Unimplemented but got Ok"),
    }
}

async fn test_query_consensus_state_unimplemented(channel: Channel, height: u64) {
    let label = "QueryConsensusState: returns Unimplemented (path not provable in v2)";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_consensus_state(QueryConsensusStateRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            revision_number: 0,
            revision_height: height,
        })
        .await;

    match resp {
        Err(status) if status.code() == Code::Unimplemented => pass(label),
        Err(status) => fail(label, format!("expected Unimplemented, got {status}")),
        Ok(_) => fail(label, "expected Unimplemented but got Ok"),
    }
}

async fn test_query_next_seq_recv_unimplemented(channel: Channel) {
    let label = "QueryNextSeqRecv: returns Unimplemented (removed in IBC v2)";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_next_seq_recv(QueryNextSeqRecvRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
        })
        .await;

    match resp {
        Err(status) if status.code() == Code::Unimplemented => pass(label),
        Err(status) => fail(label, format!("expected Unimplemented, got {status}")),
        Ok(_) => fail(label, "expected Unimplemented but got Ok"),
    }
}

async fn test_query_ibc_header(channel: Channel, height: u64) {
    let label = "QueryIbcHeader: returns serialised StellarHeader bytes";
    let mut client = StellarGatewayQueryClient::new(channel);
    let resp = client
        .query_ibc_header(QueryIbcHeaderRequest { height })
        .await;

    match resp {
        Ok(r) => {
            let body = r.into_inner();
            if body.header.is_empty() {
                fail(label, "header is empty");
                return;
            }
            pass(&format!("{label} (bytes: {})", body.header.len()));
        }
        Err(status) => fail(label, format!("rpc failed: {status}")),
    }
}

async fn test_msg_submit_signed_tx_round_trip(channel: Channel) {
    let label = "SubmitSignedTx: round-trips an empty tx_xdr";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .submit_signed_tx(SubmitSignedTxRequest { tx_xdr: vec![] })
        .await;

    match resp {
        Ok(_) => pass(label),
        Err(status) if status.code() == Code::Unimplemented => {
            pass(&format!("{label} (Unimplemented — stub OK)"))
        }
        Err(status) => fail(label, format!("rpc failed: {status}")),
    }
}

async fn test_msg_create_client_round_trip(channel: Channel) {
    let label = "CreateClient: accepts empty payload (stub)";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .create_client(MsgCreateClientRequest {
            client_state: vec![],
            consensus_state: vec![],
            signer: String::new(),
        })
        .await;
    expect_ok_or_unimplemented(label, resp);
}

async fn test_msg_update_client_round_trip(channel: Channel) {
    let label = "UpdateClient: accepts empty payload (stub)";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .update_client(MsgUpdateClientRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            header: vec![],
            signer: String::new(),
        })
        .await;
    expect_ok_or_unimplemented(label, resp);
}

async fn test_msg_register_counterparty_round_trip(channel: Channel) {
    let label = "RegisterCounterparty: accepts empty payload (stub)";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .register_counterparty(MsgRegisterCounterpartyRequest {
            client_id: DEFAULT_CLIENT_ID.to_string(),
            counterparty_client_id: "07-tendermint-0".to_string(),
            counterparty_commitment_prefix: vec![],
        })
        .await;
    expect_ok_or_unimplemented(label, resp);
}

async fn test_msg_recv_packet_round_trip(channel: Channel) {
    let label = "RecvPacket: accepts empty payload (stub)";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .recv_packet(MsgRecvPacketRequest {
            packet: vec![],
            proof: vec![],
            proof_height: 0,
            signer: String::new(),
        })
        .await;
    expect_ok_or_unimplemented(label, resp);
}

async fn test_msg_ack_packet_round_trip(channel: Channel) {
    let label = "AckPacket: accepts empty payload (stub)";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .ack_packet(MsgAckPacketRequest {
            packet: vec![],
            acknowledgement: vec![],
            proof: vec![],
            proof_height: 0,
            signer: String::new(),
        })
        .await;
    expect_ok_or_unimplemented(label, resp);
}

async fn test_msg_timeout_packet_round_trip(channel: Channel) {
    let label = "TimeoutPacket: accepts empty payload (stub)";
    let mut client = StellarGatewayMsgClient::new(channel);
    let resp = client
        .timeout_packet(MsgTimeoutPacketRequest {
            packet: vec![],
            proof: vec![],
            proof_height: 0,
            signer: String::new(),
        })
        .await;
    expect_ok_or_unimplemented(label, resp);
}

fn expect_ok_or_unimplemented<T>(label: &str, resp: Result<tonic::Response<T>, tonic::Status>) {
    match resp {
        Ok(_) => pass(label),
        Err(status) if status.code() == Code::Unimplemented => {
            pass(&format!("{label} (Unimplemented — stub OK)"))
        }
        Err(status) => fail(label, format!("rpc failed: {status}")),
    }
}
