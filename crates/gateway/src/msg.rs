use crate::{
    proto::{
        stellar_gateway_msg_server::{StellarGatewayMsg, StellarGatewayMsgServer},
        MsgAckPacketRequest, MsgAckPacketResponse, MsgCreateClientRequest, MsgCreateClientResponse,
        MsgRecvPacketRequest, MsgRecvPacketResponse, MsgRegisterCounterpartyRequest,
        MsgRegisterCounterpartyResponse, MsgTimeoutPacketRequest, MsgTimeoutPacketResponse,
        MsgUpdateClientRequest, MsgUpdateClientResponse, SubmitSignedTxRequest,
        SubmitSignedTxResponse,
    },
    rpc::RpcClient,
};
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct MsgHandler {
    pub rpc: RpcClient,
}

impl MsgHandler {
    pub fn new(rpc: RpcClient) -> Self {
        Self { rpc }
    }

    pub fn into_server(self) -> StellarGatewayMsgServer<Self> {
        StellarGatewayMsgServer::new(self)
    }
}

#[tonic::async_trait]
impl StellarGatewayMsg for MsgHandler {
    async fn submit_signed_tx(
        &self,
        _request: Request<SubmitSignedTxRequest>,
    ) -> Result<Response<SubmitSignedTxResponse>, Status> {
        unimplemented!()
    }

    async fn create_client(
        &self,
        _request: Request<MsgCreateClientRequest>,
    ) -> Result<Response<MsgCreateClientResponse>, Status> {
        unimplemented!()
    }

    async fn update_client(
        &self,
        _request: Request<MsgUpdateClientRequest>,
    ) -> Result<Response<MsgUpdateClientResponse>, Status> {
        unimplemented!()
    }

    async fn register_counterparty(
        &self,
        _request: Request<MsgRegisterCounterpartyRequest>,
    ) -> Result<Response<MsgRegisterCounterpartyResponse>, Status> {
        unimplemented!()
    }

    async fn recv_packet(
        &self,
        _request: Request<MsgRecvPacketRequest>,
    ) -> Result<Response<MsgRecvPacketResponse>, Status> {
        unimplemented!()
    }

    async fn ack_packet(
        &self,
        _request: Request<MsgAckPacketRequest>,
    ) -> Result<Response<MsgAckPacketResponse>, Status> {
        unimplemented!()
    }

    async fn timeout_packet(
        &self,
        _request: Request<MsgTimeoutPacketRequest>,
    ) -> Result<Response<MsgTimeoutPacketResponse>, Status> {
        unimplemented!()
    }
}
