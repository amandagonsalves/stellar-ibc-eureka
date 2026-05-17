use tonic::{Request, Response, Status};

use crate::proto::{
    stellar_gateway_query_server::{StellarGatewayQuery, StellarGatewayQueryServer},
    LatestHeightRequest, LatestHeightResponse, QueryClientStateRequest, QueryClientStateResponse,
    QueryConsensusStateRequest, QueryConsensusStateResponse, QueryIbcHeaderRequest,
    QueryIbcHeaderResponse, QueryNextSeqRecvRequest, QueryNextSeqRecvResponse,
    QueryPacketCommitmentRequest, QueryPacketCommitmentResponse, QueryPacketReceiptRequest,
    QueryPacketReceiptResponse,
};
use stellar_hermes_core::rpc::RpcClient;

#[derive(Clone)]
pub struct QueryHandler {
    pub rpc: RpcClient,
}

impl QueryHandler {
    pub fn new(rpc: RpcClient) -> Self {
        Self { rpc }
    }

    pub fn into_server(self) -> StellarGatewayQueryServer<Self> {
        StellarGatewayQueryServer::new(self)
    }
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
            .expect("failed to get latest ledger sequence");

        Ok(Response::new(LatestHeightResponse {
            revision_height: latest_sequence.into(),
            revision_number: 0,
        }))
    }

    async fn query_client_state(
        &self,
        _request: Request<QueryClientStateRequest>,
    ) -> Result<Response<QueryClientStateResponse>, Status> {
        unimplemented!()
    }

    async fn query_consensus_state(
        &self,
        _request: Request<QueryConsensusStateRequest>,
    ) -> Result<Response<QueryConsensusStateResponse>, Status> {
        unimplemented!()
    }

    async fn query_packet_commitment(
        &self,
        _request: Request<QueryPacketCommitmentRequest>,
    ) -> Result<Response<QueryPacketCommitmentResponse>, Status> {
        unimplemented!()
    }

    async fn query_packet_receipt(
        &self,
        _request: Request<QueryPacketReceiptRequest>,
    ) -> Result<Response<QueryPacketReceiptResponse>, Status> {
        unimplemented!()
    }

    async fn query_next_seq_recv(
        &self,
        _request: Request<QueryNextSeqRecvRequest>,
    ) -> Result<Response<QueryNextSeqRecvResponse>, Status> {
        unimplemented!()
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

        let stellar_header = crate::proto::StellarHeader {
            ledger_seq: seq,
            ledger_header_xdr: ledger.header_xdr,
            ibc_state_root: vec![0u8; 32],
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
