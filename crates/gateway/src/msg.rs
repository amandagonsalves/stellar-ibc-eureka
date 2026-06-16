use crate::proto::{
    stellar_gateway_msg_server::{StellarGatewayMsg, StellarGatewayMsgServer},
    MsgAckPacketRequest, MsgAckPacketResponse, MsgCreateClientRequest, MsgCreateClientResponse,
    MsgRecvPacketRequest, MsgRecvPacketResponse, MsgRegisterCounterpartyRequest,
    MsgRegisterCounterpartyResponse, MsgSubmitMisbehaviourRequest, MsgSubmitMisbehaviourResponse,
    MsgTimeoutPacketRequest, MsgTimeoutPacketResponse, MsgUpdateClientRequest,
    MsgUpdateClientResponse, SubmitSignedTxRequest, SubmitSignedTxResponse,
};
use soroban_client::xdr::ScVal;
use stellar_ibc_core::api_client::ApiClient;
use stellar_ibc_core::conversion::{self as cv, scval_u64};
use stellar_ibc_core::ibc::client_state::AnyClientState;
use stellar_ibc_core::ibc::consensus_state::AnyConsensusState;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct MsgHandler {
    pub api: ApiClient,
}

impl MsgHandler {
    pub fn new(api: ApiClient) -> Self {
        Self { api }
    }

    pub fn into_server(self) -> StellarGatewayMsgServer<Self> {
        StellarGatewayMsgServer::new(self)
    }

    async fn log_wallet_change(&self, packet: &ScVal) {
        let Some(value_bytes) = first_payload_value(packet) else {
            return;
        };
        let parsed: serde_json::Value = match serde_json::from_slice(&value_bytes) {
            Ok(v) => v,
            Err(_) => return,
        };
        let denom = parsed.get("denom").and_then(|v| v.as_str());
        let sender = parsed.get("sender").and_then(|v| v.as_str());
        let amount = parsed
            .get("amount")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i128>().ok());
        let (Some(denom), Some(sender), Some(amount)) = (denom, sender, amount) else {
            return;
        };

        match self.api.get_transfer_balance(denom, sender).await {
            Ok(new_balance) => {
                let current_balance = new_balance.saturating_add(amount);
                tracing::info!(
                    denom,
                    sender,
                    current_balance = %current_balance,
                    new_balance = %new_balance,
                    moved = %amount,
                    "[cosmos→stellar] wallet settled — sender escrow reflects the transfer"
                );
            }
            Err(error) => {
                tracing::debug!(%error, "wallet balance read failed (demo log)");
            }
        }
    }

    async fn prepare_msg_tx(
        &self,
        signer: &str,
        method: &str,
        args: Vec<ScVal>,
    ) -> Result<Vec<u8>, Status> {
        tracing::debug!(method, signer, args = args.len(), "prepare_router via api");
        self.api
            .build_unsigned_tx(signer, method, args)
            .await
            .map_err(|error| {
                tracing::error!(%error, method, "invoke failed");
                Status::internal(format!("invoke({method}): {error}"))
            })
    }
}

fn scval_string(s: &str) -> Result<ScVal, Status> {
    cv::scval_string(s).map_err(|e| Status::invalid_argument(e.to_string()))
}

fn scval_bytes(b: &[u8]) -> Result<ScVal, Status> {
    cv::scval_bytes(b).map_err(|e| Status::invalid_argument(e.to_string()))
}

fn scval_vec_of_bytes(items: &[Vec<u8>]) -> Result<ScVal, Status> {
    cv::scval_vec_of_bytes(items).map_err(|e| Status::invalid_argument(e.to_string()))
}

fn decode_packet_scval(bytes: &[u8]) -> Result<ScVal, Status> {
    cv::scval_from_xdr(bytes)
        .map_err(|e| Status::invalid_argument(format!("packet ScVal XDR decode: {e}")))
}

fn first_payload_value(packet: &ScVal) -> Option<Vec<u8>> {
    let payloads = cv::scval_field(cv::scval_as_map(packet)?, "payloads")?;
    let ScVal::Vec(Some(items)) = payloads else {
        return None;
    };
    let first = items.0.first()?;
    let value = cv::scval_field(cv::scval_as_map(first)?, "value")?;
    cv::scval_as_bytes(value)
}

#[tonic::async_trait]
impl StellarGatewayMsg for MsgHandler {
    #[tracing::instrument(skip(self, request), name = "grpc.submit_signed_tx")]
    async fn submit_signed_tx(
        &self,
        request: Request<SubmitSignedTxRequest>,
    ) -> Result<Response<SubmitSignedTxResponse>, Status> {
        let tx_xdr = request.into_inner().tx_xdr;
        tracing::debug!(tx_bytes = tx_xdr.len(), "gRPC SubmitSignedTx");
        let submitted = self.api.submit_and_wait(&tx_xdr).await.map_err(|error| {
            tracing::error!(%error, "submit_and_wait_for_result failed");
            Status::internal(format!("submit_and_wait: {error}"))
        })?;
        let return_value = submitted
            .return_value
            .and_then(|v| cv::scval_to_xdr(&v).ok())
            .unwrap_or_default();
        tracing::info!(tx_hash = %submitted.hash, "[gateway] tx submitted");
        Ok(Response::new(SubmitSignedTxResponse {
            tx_hash: submitted.hash,
            events: Vec::new(),
            return_value,
        }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.create_client")]
    async fn create_client(
        &self,
        request: Request<MsgCreateClientRequest>,
    ) -> Result<Response<MsgCreateClientResponse>, Status> {
        let req = request.into_inner();
        if req.client_type.is_empty() {
            return Err(Status::invalid_argument(
                "MsgCreateClientRequest.client_type is required",
            ));
        }
        tracing::info!(
            client_type = %req.client_type,
            height = req.height,
            "[gateway] CreateClient"
        );
        let client_state = AnyClientState::decode_value(&req.client_state)
            .map_err(|e| Status::invalid_argument(format!("decode client state: {e}")))?;

        let height = client_state.latest_height();
        tracing::debug!(
            chain_id = %client_state.chain_id(),
            revision_number = client_state.revision_number(),
            latest_height = height,
            "decoded tendermint client state"
        );
        let client_state_xdr = client_state
            .to_soroban_xdr()
            .map_err(|e| Status::internal(format!("client state to soroban xdr: {e}")))?;

        let consensus_state = AnyConsensusState::decode_value(&req.consensus_state)
            .map_err(|e| Status::invalid_argument(format!("decode consensus state: {e}")))?;
        let consensus_state_xdr = consensus_state
            .to_soroban_xdr()
            .map_err(|e| Status::internal(format!("consensus state to soroban xdr: {e}")))?;

        let args = vec![
            scval_string(&req.client_type)?,
            scval_bytes(&client_state_xdr)?,
            scval_bytes(&consensus_state_xdr)?,
            scval_u64(height),
        ];
        let tx_xdr = self.prepare_msg_tx(&req.signer, "create_client", args).await?;
        tracing::debug!(tx_bytes = tx_xdr.len(), "create_client prepared (unsigned)");
        Ok(Response::new(MsgCreateClientResponse {
            client_id: String::new(),
            tx_xdr,
        }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.update_client")]
    async fn update_client(
        &self,
        request: Request<MsgUpdateClientRequest>,
    ) -> Result<Response<MsgUpdateClientResponse>, Status> {
        let req = request.into_inner();
        if req.client_id.is_empty() {
            return Err(Status::invalid_argument(
                "MsgUpdateClientRequest.client_id is required",
            ));
        }
        tracing::info!(
            client_id = %req.client_id,
            "[gateway] UpdateClient"
        );

        let header_xdr = if req.client_id.starts_with("07-tendermint") {
            stellar_ibc_core::ibc::soroban::tendermint_header_to_soroban_xdr(&req.header).map_err(
                |e| Status::invalid_argument(format!("tendermint header to soroban xdr: {e}")),
            )?
        } else {
            req.header.clone()
        };

        let args = vec![scval_string(&req.client_id)?, scval_bytes(&header_xdr)?];
        let tx_xdr = self.prepare_msg_tx(&req.signer, "update_client", args).await?;
        Ok(Response::new(MsgUpdateClientResponse { tx_xdr }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.register_counterparty")]
    async fn register_counterparty(
        &self,
        request: Request<MsgRegisterCounterpartyRequest>,
    ) -> Result<Response<MsgRegisterCounterpartyResponse>, Status> {
        let req = request.into_inner();
        if req.client_id.is_empty() || req.counterparty_client_id.is_empty() {
            return Err(Status::invalid_argument(
                "client_id and counterparty_client_id are required",
            ));
        }
        tracing::info!(
            client_id = %req.client_id,
            counterparty_client_id = %req.counterparty_client_id,
            "[gateway] RegisterCounterparty"
        );
        let args = vec![
            scval_string(&req.client_id)?,
            scval_string(&req.counterparty_client_id)?,
            scval_vec_of_bytes(&req.counterparty_commitment_prefix)?,
        ];
        let tx_xdr = self.prepare_msg_tx("", "register_counterparty", args).await?;
        Ok(Response::new(MsgRegisterCounterpartyResponse { tx_xdr }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.recv_packet")]
    async fn recv_packet(
        &self,
        request: Request<MsgRecvPacketRequest>,
    ) -> Result<Response<MsgRecvPacketResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            proof_height = req.proof_height,
            "[gateway] RecvPacket"
        );
        let args = vec![
            decode_packet_scval(&req.packet)?,
            scval_bytes(&req.proof)?,
            scval_u64(req.proof_height),
        ];
        let tx_xdr = self.prepare_msg_tx(&req.signer, "recv_packet", args).await?;
        Ok(Response::new(MsgRecvPacketResponse { tx_xdr }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.ack_packet")]
    async fn ack_packet(
        &self,
        request: Request<MsgAckPacketRequest>,
    ) -> Result<Response<MsgAckPacketResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            ack_bytes = req.acknowledgement.len(),
            proof_height = req.proof_height,
            "[cosmos→stellar] AckPacket → router.acknowledge_packet"
        );
        let packet_scval = decode_packet_scval(&req.packet)?;
        self.log_wallet_change(&packet_scval).await;
        let acks = scval_vec_of_bytes(&[req.acknowledgement])?;
        let args = vec![
            packet_scval,
            acks,
            scval_bytes(&req.proof)?,
            scval_u64(req.proof_height),
        ];
        let tx_xdr = self.prepare_msg_tx(&req.signer, "acknowledge_packet", args).await?;
        Ok(Response::new(MsgAckPacketResponse { tx_xdr }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.timeout_packet")]
    async fn timeout_packet(
        &self,
        request: Request<MsgTimeoutPacketRequest>,
    ) -> Result<Response<MsgTimeoutPacketResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            proof_height = req.proof_height,
            "[gateway] TimeoutPacket"
        );
        let args = vec![
            decode_packet_scval(&req.packet)?,
            scval_bytes(&req.proof)?,
            scval_u64(req.proof_height),
        ];
        let tx_xdr = self.prepare_msg_tx(&req.signer, "timeout_packet", args).await?;
        Ok(Response::new(MsgTimeoutPacketResponse { tx_xdr }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.submit_misbehaviour")]
    async fn submit_misbehaviour(
        &self,
        request: Request<MsgSubmitMisbehaviourRequest>,
    ) -> Result<Response<MsgSubmitMisbehaviourResponse>, Status> {
        let req = request.into_inner();
        if req.client_id.is_empty() {
            return Err(Status::invalid_argument(
                "MsgSubmitMisbehaviourRequest.client_id is required",
            ));
        }
        tracing::info!(
            client_id = %req.client_id,
            "[gateway] SubmitMisbehaviour"
        );
        let args = vec![
            scval_string(&req.client_id)?,
            scval_bytes(&req.client_message)?,
        ];
        let tx_xdr = self.prepare_msg_tx(&req.signer, "update_client", args).await?;
        Ok(Response::new(MsgSubmitMisbehaviourResponse { tx_xdr }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_client::xdr::ScVec;

    fn handler() -> MsgHandler {
        MsgHandler::new(ApiClient::new("http://127.0.0.1:8101"))
    }

    #[tokio::test]
    async fn submit_misbehaviour_rejects_missing_client_id() {
        let h = handler();
        let req = Request::new(MsgSubmitMisbehaviourRequest {
            client_id: String::new(),
            client_message: vec![1, 2, 3],
            signer: String::new(),
        });
        let err = h.submit_misbehaviour(req).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::InvalidArgument);
        assert!(err.message().contains("client_id"));
    }

    #[test]
    fn scval_helpers_produce_expected_variants() {
        let s = scval_string("transfer").unwrap();
        assert!(matches!(s, ScVal::String(_)));

        let b = scval_bytes(b"abc").unwrap();
        assert!(matches!(b, ScVal::Bytes(_)));

        let u = scval_u64(42);
        assert!(matches!(u, ScVal::U64(42)));

        let v = scval_vec_of_bytes(&[b"ibc".to_vec(), b"\x01\x02".to_vec()]).unwrap();
        let inner = match v {
            ScVal::Vec(Some(ScVec(items))) => items,
            _ => panic!("expected ScVal::Vec(Some(_))"),
        };
        assert_eq!(inner.len(), 2);
        assert!(matches!(inner[0], ScVal::Bytes(_)));
    }
}
