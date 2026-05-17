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
        _request: Request<QueryIbcHeaderRequest>,
    ) -> Result<Response<QueryIbcHeaderResponse>, Status> {
        unimplemented!()
    }
}
