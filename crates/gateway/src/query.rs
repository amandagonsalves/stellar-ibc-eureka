use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::proto::{
    stellar_gateway_query_server::{StellarGatewayQuery, StellarGatewayQueryServer},
    LatestHeightRequest, LatestHeightResponse, QueryAcknowledgementRequest,
    QueryAcknowledgementResponse, QueryClientStateRequest, QueryClientStateResponse,
    QueryConsensusStateRequest, QueryConsensusStateResponse, QueryIbcHeaderRequest,
    QueryIbcHeaderResponse, QueryNextSeqRecvRequest, QueryNextSeqRecvResponse,
    QueryPacketCommitmentRequest, QueryPacketCommitmentResponse, QueryPacketReceiptRequest,
    QueryPacketReceiptResponse,
};
use crate::state_tracker::{PathLookup, StateTracker};
use stellar_hermes_core::rpc::RpcClient;

const PACKET_COMMITMENT_DISCRIMINATOR: u8 = 0x01;
const PACKET_RECEIPT_DISCRIMINATOR: u8 = 0x02;
const ACK_COMMITMENT_DISCRIMINATOR: u8 = 0x03;

#[derive(Clone)]
pub struct QueryHandler {
    pub rpc: RpcClient,
    pub tracker: Arc<Mutex<StateTracker>>,
}

impl QueryHandler {
    pub fn new(rpc: RpcClient, tracker: Arc<Mutex<StateTracker>>) -> Self {
        Self { rpc, tracker }
    }

    pub fn into_server(self) -> StellarGatewayQueryServer<Self> {
        StellarGatewayQueryServer::new(self)
    }
}

fn v2_path(client_id: &str, discriminator: u8, sequence: u64) -> Vec<u8> {
    let client_bytes = client_id.as_bytes();
    let mut key = Vec::with_capacity(client_bytes.len() + 1 + 8);
    key.extend_from_slice(client_bytes);
    key.push(discriminator);
    key.extend_from_slice(&sequence.to_be_bytes());
    key
}

#[tonic::async_trait]
impl StellarGatewayQuery for QueryHandler {
    async fn latest_height(
        &self,
        _request: Request<LatestHeightRequest>,
    ) -> Result<Response<LatestHeightResponse>, Status> {
        let latest_sequence: u32 = self
            .rpc
            .latest_ledger_sequence()
            .await
            .map_err(|e| Status::internal(format!("latest_ledger_sequence failed: {e}")))?;

        Ok(Response::new(LatestHeightResponse {
            revision_height: latest_sequence.into(),
            revision_number: 0,
        }))
    }

    async fn query_client_state(
        &self,
        _request: Request<QueryClientStateRequest>,
    ) -> Result<Response<QueryClientStateResponse>, Status> {
        Err(Status::unimplemented(
            "ClientState path is non-provable in IBC v2",
        ))
    }

    async fn query_consensus_state(
        &self,
        _request: Request<QueryConsensusStateRequest>,
    ) -> Result<Response<QueryConsensusStateResponse>, Status> {
        Err(Status::unimplemented(
            "ConsensusState path is non-provable in IBC v2",
        ))
    }

    async fn query_packet_commitment(
        &self,
        request: Request<QueryPacketCommitmentRequest>,
    ) -> Result<Response<QueryPacketCommitmentResponse>, Status> {
        let req = request.into_inner();
        let seq = u32::try_from(req.height).map_err(|_| {
            Status::invalid_argument(format!("height {} does not fit in u32", req.height))
        })?;
        let key = v2_path(
            &req.client_id,
            PACKET_COMMITMENT_DISCRIMINATOR,
            req.sequence,
        );

        let lookup = self
            .tracker
            .lock()
            .await
            .proof_for_path(seq, &key)
            .await
            .map_err(|e| Status::internal(format!("proof_for_path failed: {e}")))?;

        let (commitment, proof) = match lookup {
            PathLookup::Found {
                value_hash,
                proof_bytes,
            } => (value_hash.to_vec(), proof_bytes),
            PathLookup::Absent { proof_bytes } => (Vec::new(), proof_bytes),
        };

        Ok(Response::new(QueryPacketCommitmentResponse {
            commitment,
            proof,
            proof_height: req.height,
        }))
    }

    async fn query_packet_receipt(
        &self,
        request: Request<QueryPacketReceiptRequest>,
    ) -> Result<Response<QueryPacketReceiptResponse>, Status> {
        let req = request.into_inner();
        let seq = u32::try_from(req.height).map_err(|_| {
            Status::invalid_argument(format!("height {} does not fit in u32", req.height))
        })?;
        let key = v2_path(&req.client_id, PACKET_RECEIPT_DISCRIMINATOR, req.sequence);

        let lookup = self
            .tracker
            .lock()
            .await
            .proof_for_path(seq, &key)
            .await
            .map_err(|e| Status::internal(format!("proof_for_path failed: {e}")))?;

        let (received, proof) = match lookup {
            PathLookup::Found { proof_bytes, .. } => (true, proof_bytes),
            PathLookup::Absent { proof_bytes } => (false, proof_bytes),
        };

        Ok(Response::new(QueryPacketReceiptResponse {
            received,
            proof,
            proof_height: req.height,
        }))
    }

    async fn query_acknowledgement(
        &self,
        request: Request<QueryAcknowledgementRequest>,
    ) -> Result<Response<QueryAcknowledgementResponse>, Status> {
        let req = request.into_inner();
        let seq = u32::try_from(req.height).map_err(|_| {
            Status::invalid_argument(format!("height {} does not fit in u32", req.height))
        })?;
        let key = v2_path(&req.client_id, ACK_COMMITMENT_DISCRIMINATOR, req.sequence);

        let lookup = self
            .tracker
            .lock()
            .await
            .proof_for_path(seq, &key)
            .await
            .map_err(|e| Status::internal(format!("proof_for_path failed: {e}")))?;

        let (acknowledgement, proof) = match lookup {
            PathLookup::Found {
                value_hash,
                proof_bytes,
            } => (value_hash.to_vec(), proof_bytes),
            PathLookup::Absent { proof_bytes } => (Vec::new(), proof_bytes),
        };

        Ok(Response::new(QueryAcknowledgementResponse {
            acknowledgement,
            proof,
            proof_height: req.height,
        }))
    }

    async fn query_next_seq_recv(
        &self,
        _request: Request<QueryNextSeqRecvRequest>,
    ) -> Result<Response<QueryNextSeqRecvResponse>, Status> {
        Err(Status::unimplemented(
            "QueryNextSeqRecv: nextSequenceSend path was removed in IBC v2",
        ))
    }

    async fn query_ibc_header(
        &self,
        request: Request<QueryIbcHeaderRequest>,
    ) -> Result<Response<QueryIbcHeaderResponse>, Status> {
        use prost::Message as _;
        use soroban_client::xdr::{LedgerHeader, Limits, ReadXdr, StellarValueExt, WriteXdr};

        let seq = request.into_inner().height as u32;

        let ledger = self
            .rpc
            .get_ledger(seq)
            .await
            .map_err(|e| Status::internal(format!("getLedger failed: {e}")))?;

        let header = LedgerHeader::from_xdr(&ledger.header_xdr, Limits::none())
            .map_err(|e| Status::internal(format!("LedgerHeader XDR decode: {e}")))?;

        let (scp_node_id, scp_signature) = match header.scp_value.ext {
            StellarValueExt::Signed(sig) => {
                let node_id_xdr = sig
                    .node_id
                    .to_xdr(Limits::none())
                    .map_err(|e| Status::internal(format!("NodeId XDR encode: {e}")))?;
                (node_id_xdr, sig.signature.to_vec())
            }
            StellarValueExt::Basic => (vec![], vec![]),
        };

        let ibc_state_root = self
            .tracker
            .lock()
            .await
            .root_at(seq)
            .await
            .map_err(|e| Status::internal(format!("state root computation failed: {e}")))?;

        let stellar_header = crate::proto::StellarHeader {
            ledger_seq: seq,
            ledger_header_xdr: ledger.header_xdr,
            ibc_state_root: ibc_state_root.to_vec(),
            scp_node_id,
            scp_signature,
        };

        let mut header_bytes = vec![];
        stellar_header
            .encode(&mut header_bytes)
            .map_err(|e| Status::internal(format!("StellarHeader encode: {e}")))?;

        Ok(Response::new(QueryIbcHeaderResponse {
            header: header_bytes,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v2_path_layout_matches_ics24() {
        let key = v2_path("10-stellar-0", PACKET_COMMITMENT_DISCRIMINATOR, 0x1234);
        let id = b"10-stellar-0";
        assert_eq!(&key[..id.len()], id);
        assert_eq!(key[id.len()], 0x01);
        assert_eq!(&key[id.len() + 1..], &0x1234u64.to_be_bytes());
    }

    #[test]
    fn discriminator_bytes_match_spec() {
        assert_eq!(PACKET_COMMITMENT_DISCRIMINATOR, 0x01);
        assert_eq!(PACKET_RECEIPT_DISCRIMINATOR, 0x02);
        assert_eq!(ACK_COMMITMENT_DISCRIMINATOR, 0x03);
    }
}
