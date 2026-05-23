use crate::proto::{
    stellar_gateway_msg_server::{StellarGatewayMsg, StellarGatewayMsgServer},
    MsgAckPacketRequest, MsgAckPacketResponse, MsgCreateClientRequest, MsgCreateClientResponse,
    MsgRecvPacketRequest, MsgRecvPacketResponse, MsgRegisterCounterpartyRequest,
    MsgRegisterCounterpartyResponse, MsgSubmitMisbehaviourRequest, MsgSubmitMisbehaviourResponse,
    MsgTimeoutPacketRequest, MsgTimeoutPacketResponse, MsgUpdateClientRequest,
    MsgUpdateClientResponse, SubmitSignedTxRequest, SubmitSignedTxResponse,
};
use soroban_client::{
    contract::{ContractBehavior, Contracts},
    keypair::{Keypair, KeypairBehavior},
    transaction::TransactionBehavior,
    transaction_builder::{TransactionBuilder, TransactionBuilderBehavior, TIMEOUT_INFINITE},
    xdr::{Limits, ReadXdr, ScBytes, ScString, ScVal, ScVec, StringM, VecM, WriteXdr},
};
use stellar_ibc_core::rpc::RpcClient;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct MsgHandler {
    pub rpc: RpcClient,
    pub ibc_contract_id: String,
    pub signing_key: String,
    pub network_passphrase: String,
}

impl MsgHandler {
    pub fn new(
        rpc: RpcClient,
        ibc_contract_id: String,
        signing_key: String,
        network_passphrase: String,
    ) -> Self {
        Self {
            rpc,
            ibc_contract_id,
            signing_key,
            network_passphrase,
        }
    }

    pub fn into_server(self) -> StellarGatewayMsgServer<Self> {
        StellarGatewayMsgServer::new(self)
    }

    async fn invoke_router(&self, method: &str, args: Vec<ScVal>) -> Result<String, Status> {
        if self.ibc_contract_id.is_empty() {
            return Err(Status::failed_precondition(
                "gateway IBC_CONTRACT_ID is not configured",
            ));
        }
        if self.signing_key.is_empty() {
            return Err(Status::failed_precondition(
                "gateway STELLAR_SIGNING_KEY is not configured",
            ));
        }

        stellar_strkey::ed25519::PrivateKey::from_string(&self.signing_key)
            .map_err(|e| Status::internal(format!("signing key parse failed: {e}")))?;
        let keypair = Keypair::from_secret(&self.signing_key)
            .map_err(|e| Status::internal(format!("signing key parse failed: {e}")))?;
        let public_key = keypair.public_key();

        let mut account = self
            .rpc
            .server
            .get_account(&public_key)
            .await
            .map_err(|e| Status::internal(format!("get_account({public_key}): {e:?}")))?;

        let contract = Contracts::new(&self.ibc_contract_id)
            .map_err(|e| Status::invalid_argument(format!("invalid IBC_CONTRACT_ID: {e}")))?;
        let op = contract.call(method, Some(args));

        let tx_to_simulate =
            TransactionBuilder::new(&mut account, &self.network_passphrase, None)
                .fee(100_u32)
                .add_operation(op)
                .set_timeout(TIMEOUT_INFINITE)
                .map_err(|e| Status::internal(format!("set_timeout: {e}")))?
                .build();

        let mut tx = self
            .rpc
            .server
            .prepare_transaction(&tx_to_simulate)
            .await
            .map_err(|e| Status::internal(format!("prepare_transaction({method}): {e:?}")))?;

        tx.sign(&[keypair]);

        let envelope = tx
            .to_envelope()
            .map_err(|e| Status::internal(format!("to_envelope: {e}")))?;
        let envelope_bytes = envelope
            .to_xdr(Limits::none())
            .map_err(|e| Status::internal(format!("envelope XDR encode: {e}")))?;

        self.rpc
            .submit_and_wait(&envelope_bytes)
            .await
            .map_err(|e| Status::internal(format!("submit_and_wait({method}): {e}")))
    }
}

fn scval_string(s: &str) -> Result<ScVal, Status> {
    let sm = StringM::<{ u32::MAX }>::try_from(s.as_bytes())
        .map_err(|e| Status::invalid_argument(format!("invalid string for ScVal: {e}")))?;
    Ok(ScVal::String(ScString(sm)))
}

fn scval_bytes(b: &[u8]) -> Result<ScVal, Status> {
    let bm = b
        .to_vec()
        .try_into()
        .map_err(|e| Status::invalid_argument(format!("invalid bytes for ScVal: {e}")))?;
    Ok(ScVal::Bytes(ScBytes(bm)))
}

fn scval_u64(v: u64) -> ScVal {
    ScVal::U64(v)
}

fn scval_vec_of_bytes(items: &[Vec<u8>]) -> Result<ScVal, Status> {
    let inner: Result<Vec<ScVal>, Status> = items.iter().map(|b| scval_bytes(b)).collect();
    let vecm = VecM::<ScVal>::try_from(inner?)
        .map_err(|e| Status::invalid_argument(format!("invalid Vec<Bytes>: {e}")))?;
    Ok(ScVal::Vec(Some(ScVec(vecm))))
}

fn decode_packet_scval(bytes: &[u8]) -> Result<ScVal, Status> {
    ScVal::from_xdr(bytes, Limits::none())
        .map_err(|e| Status::invalid_argument(format!("packet ScVal XDR decode: {e}")))
}

#[tonic::async_trait]
impl StellarGatewayMsg for MsgHandler {
    async fn submit_signed_tx(
        &self,
        request: Request<SubmitSignedTxRequest>,
    ) -> Result<Response<SubmitSignedTxResponse>, Status> {
        let tx_xdr = request.into_inner().tx_xdr;
        let tx_hash = self
            .rpc
            .submit_and_wait(&tx_xdr)
            .await
            .map_err(|e| Status::internal(format!("submit_and_wait: {e}")))?;
        Ok(Response::new(SubmitSignedTxResponse {
            tx_hash,
            events: Vec::new(),
        }))
    }

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
        let args = vec![
            scval_string(&req.client_type)?,
            scval_bytes(&req.client_state)?,
            scval_bytes(&req.consensus_state)?,
            scval_u64(req.height),
        ];
        let _tx_hash = self.invoke_router("create_client", args).await?;
        Ok(Response::new(MsgCreateClientResponse {
            client_id: String::new(),
        }))
    }

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
        let args = vec![scval_string(&req.client_id)?, scval_bytes(&req.header)?];
        let _tx_hash = self.invoke_router("update_client", args).await?;
        Ok(Response::new(MsgUpdateClientResponse {}))
    }

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
        let args = vec![
            scval_string(&req.client_id)?,
            scval_string(&req.counterparty_client_id)?,
            scval_vec_of_bytes(&req.counterparty_commitment_prefix)?,
        ];
        let _tx_hash = self.invoke_router("register_counterparty", args).await?;
        Ok(Response::new(MsgRegisterCounterpartyResponse {}))
    }

    async fn recv_packet(
        &self,
        request: Request<MsgRecvPacketRequest>,
    ) -> Result<Response<MsgRecvPacketResponse>, Status> {
        let req = request.into_inner();
        let args = vec![
            decode_packet_scval(&req.packet)?,
            scval_bytes(&req.proof)?,
            scval_u64(req.proof_height),
        ];
        let _tx_hash = self.invoke_router("recv_packet", args).await?;
        Ok(Response::new(MsgRecvPacketResponse {}))
    }

    async fn ack_packet(
        &self,
        request: Request<MsgAckPacketRequest>,
    ) -> Result<Response<MsgAckPacketResponse>, Status> {
        let req = request.into_inner();
        let acks = scval_vec_of_bytes(&[req.acknowledgement])?;
        let args = vec![
            decode_packet_scval(&req.packet)?,
            acks,
            scval_bytes(&req.proof)?,
            scval_u64(req.proof_height),
        ];
        let _tx_hash = self.invoke_router("acknowledge_packet", args).await?;
        Ok(Response::new(MsgAckPacketResponse {}))
    }

    async fn timeout_packet(
        &self,
        request: Request<MsgTimeoutPacketRequest>,
    ) -> Result<Response<MsgTimeoutPacketResponse>, Status> {
        let req = request.into_inner();
        let args = vec![
            decode_packet_scval(&req.packet)?,
            scval_bytes(&req.proof)?,
            scval_u64(req.proof_height),
        ];
        let _tx_hash = self.invoke_router("timeout_packet", args).await?;
        Ok(Response::new(MsgTimeoutPacketResponse {}))
    }

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
        let args = vec![
            scval_string(&req.client_id)?,
            scval_bytes(&req.client_message)?,
        ];
        let _tx_hash = self.invoke_router("update_client", args).await?;
        Ok(Response::new(MsgSubmitMisbehaviourResponse {}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TESTNET_URL: &str = "https://soroban-testnet.stellar.org";
    const NETWORK_PASSPHRASE: &str = "Test SDF Network ; September 2015";
    const VALID_CONTRACT_ID: &str =
        "CBHJI5KZOZUPE7ADDBEYDC6VHZAQI7HHK6JIFW2M67KBRY36OYPVFXGB";

    fn rpc() -> RpcClient {
        RpcClient::new(TESTNET_URL).unwrap()
    }

    fn handler(contract: &str, signing_key: &str) -> MsgHandler {
        MsgHandler::new(
            rpc(),
            contract.to_string(),
            signing_key.to_string(),
            NETWORK_PASSPHRASE.to_string(),
        )
    }

    #[tokio::test]
    async fn invoke_router_requires_contract_id() {
        let h = handler("", "SAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
        let err = h.invoke_router("noop", vec![]).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
        assert!(err.message().contains("IBC_CONTRACT_ID"));
    }

    #[tokio::test]
    async fn invoke_router_requires_signing_key() {
        let h = handler(VALID_CONTRACT_ID, "");
        let err = h.invoke_router("noop", vec![]).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::FailedPrecondition);
        assert!(err.message().contains("STELLAR_SIGNING_KEY"));
    }

    #[tokio::test]
    async fn invoke_router_rejects_invalid_signing_key() {
        let h = handler(VALID_CONTRACT_ID, "not-a-real-strkey");
        let err = h.invoke_router("noop", vec![]).await.unwrap_err();
        assert_eq!(err.code(), tonic::Code::Internal);
        assert!(err.message().contains("signing key parse failed"));
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
