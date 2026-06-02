use std::sync::Arc;

use tokio::sync::Mutex;
use tonic::{Request, Response, Status};

use crate::proto::{
    stellar_gateway_query_server::{StellarGatewayQuery, StellarGatewayQueryServer},
    EventsRequest, EventsResponse, GatewayContractEvent, IdentifiedClientState,
    LatestHeightRequest, LatestHeightResponse, QueryAcknowledgementRequest,
    QueryAcknowledgementResponse, QueryClientStateRequest, QueryClientStateResponse,
    QueryClientStatesRequest, QueryClientStatesResponse, QueryConsensusStateRequest,
    QueryConsensusStateResponse, QueryIbcHeaderRequest, QueryIbcHeaderResponse,
    QueryNextSeqRecvRequest, QueryNextSeqRecvResponse, QueryPacketCommitmentRequest,
    QueryPacketCommitmentResponse, QueryPacketReceiptRequest, QueryPacketReceiptResponse,
};
use crate::state_tracker::{PathLookup, StateTracker};
use stellar_ibc_core::api_client::{ApiClient, EventCursor};
use stellar_ibc_core::commitment::{
    ack_commitment_path, packet_commitment_path, packet_receipt_path,
};

#[derive(Clone)]
pub struct QueryHandler {
    pub api: ApiClient,
    pub tracker: Arc<Mutex<StateTracker>>,
    pub ibc_contract_id: Option<String>,
}

impl QueryHandler {
    pub fn new(
        api: ApiClient,
        tracker: Arc<Mutex<StateTracker>>,
        ibc_contract_id: Option<String>,
    ) -> Self {
        Self {
            api,
            tracker,
            ibc_contract_id,
        }
    }

    pub fn into_server(self) -> StellarGatewayQueryServer<Self> {
        StellarGatewayQueryServer::new(self)
    }
}

#[tonic::async_trait]
impl StellarGatewayQuery for QueryHandler {
    #[tracing::instrument(skip(self, _request), name = "grpc.latest_height")]
    async fn latest_height(
        &self,
        _request: Request<LatestHeightRequest>,
    ) -> Result<Response<LatestHeightResponse>, Status> {
        let latest_sequence: u32 = self.api.get_latest_ledger().await.map_err(|error| {
            tracing::error!(%error, "get_latest_ledger failed");
            Status::internal(format!("get_latest_ledger failed: {error}"))
        })?;

        tracing::debug!(revision_height = latest_sequence, "latest height");
        Ok(Response::new(LatestHeightResponse {
            revision_height: latest_sequence.into(),
            revision_number: 0,
        }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.query_client_state")]
    async fn query_client_state(
        &self,
        request: Request<QueryClientStateRequest>,
    ) -> Result<Response<QueryClientStateResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(client_id = %req.client_id, "gRPC QueryClientState");

        let xdr = self
            .api
            .get_client_state_xdr(&req.client_id)
            .await
            .map_err(|error| Status::internal(format!("get client state: {error}")))?;

        let cs = stellar_ibc_core::ibc::client_state::AnyClientState::from_soroban_xdr(&xdr)
            .map_err(|error| Status::internal(format!("decode soroban client state: {error}")))?;

        let client_state =
            ibc::primitives::proto::Protobuf::<ibc::primitives::proto::Any>::encode_vec(cs);

        Ok(Response::new(QueryClientStateResponse {
            client_state,
            proof: Vec::new(),
            proof_height: req.height,
        }))
    }

    #[tracing::instrument(skip(self, _request), name = "grpc.query_client_states")]
    async fn query_client_states(
        &self,
        _request: Request<QueryClientStatesRequest>,
    ) -> Result<Response<QueryClientStatesResponse>, Status> {
        tracing::info!("gRPC QueryClientStates");

        let ids = self
            .api
            .list_client_ids()
            .await
            .map_err(|error| Status::internal(format!("list clients: {error}")))?;

        let mut client_states = Vec::new();
        for client_id in ids {
            let xdr = match self.api.get_client_state_xdr(&client_id).await {
                Ok(xdr) => xdr,
                Err(error) => {
                    tracing::warn!(%client_id, %error, "skipping client: state fetch failed");
                    continue;
                }
            };
            let cs =
                match stellar_ibc_core::ibc::client_state::AnyClientState::from_soroban_xdr(&xdr) {
                    Ok(cs) => cs,
                    Err(error) => {
                        tracing::warn!(%client_id, %error, "skipping client: decode failed");
                        continue;
                    }
                };
            let client_state =
                ibc::primitives::proto::Protobuf::<ibc::primitives::proto::Any>::encode_vec(cs);
            client_states.push(IdentifiedClientState {
                client_id,
                client_state,
            });
        }

        Ok(Response::new(QueryClientStatesResponse { client_states }))
    }

    #[tracing::instrument(skip(self, _request), name = "grpc.query_consensus_state")]
    async fn query_consensus_state(
        &self,
        _request: Request<QueryConsensusStateRequest>,
    ) -> Result<Response<QueryConsensusStateResponse>, Status> {
        tracing::debug!("gRPC QueryConsensusState (unimplemented in v2)");
        Err(Status::unimplemented(
            "ConsensusState path is non-provable in IBC v2",
        ))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.query_packet_commitment")]
    async fn query_packet_commitment(
        &self,
        request: Request<QueryPacketCommitmentRequest>,
    ) -> Result<Response<QueryPacketCommitmentResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            client_id = %req.client_id,
            sequence = req.sequence,
            height = req.height,
            "gRPC QueryPacketCommitment"
        );
        let seq = u32::try_from(req.height).map_err(|_| {
            Status::invalid_argument(format!("height {} does not fit in u32", req.height))
        })?;
        let key = packet_commitment_path(req.client_id.as_bytes(), req.sequence);

        let lookup = self
            .tracker
            .lock()
            .await
            .proof_for_path(seq, &key)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, sequence = req.sequence, "proof_for_path failed");
                Status::internal(format!("proof_for_path failed: {e}"))
            })?;

        let (commitment, proof) = match lookup {
            PathLookup::Found {
                value_hash,
                proof_bytes,
            } => (value_hash.to_vec(), proof_bytes),
            PathLookup::Absent { proof_bytes } => (Vec::new(), proof_bytes),
        };
        tracing::info!(
            client_id = %req.client_id,
            sequence = req.sequence,
            commitment_bytes = commitment.len(),
            proof_bytes = proof.len(),
            present = !commitment.is_empty(),
            "served packet commitment proof"
        );

        Ok(Response::new(QueryPacketCommitmentResponse {
            commitment,
            proof,
            proof_height: req.height,
        }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.query_packet_receipt")]
    async fn query_packet_receipt(
        &self,
        request: Request<QueryPacketReceiptRequest>,
    ) -> Result<Response<QueryPacketReceiptResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            client_id = %req.client_id,
            sequence = req.sequence,
            height = req.height,
            "gRPC QueryPacketReceipt"
        );
        let seq = u32::try_from(req.height).map_err(|_| {
            Status::invalid_argument(format!("height {} does not fit in u32", req.height))
        })?;
        let key = packet_receipt_path(req.client_id.as_bytes(), req.sequence);

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
        tracing::info!(received, proof_bytes = proof.len(), "served packet receipt");

        Ok(Response::new(QueryPacketReceiptResponse {
            received,
            proof,
            proof_height: req.height,
        }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.query_acknowledgement")]
    async fn query_acknowledgement(
        &self,
        request: Request<QueryAcknowledgementRequest>,
    ) -> Result<Response<QueryAcknowledgementResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(
            client_id = %req.client_id,
            sequence = req.sequence,
            height = req.height,
            "gRPC QueryAcknowledgement"
        );
        let seq = u32::try_from(req.height).map_err(|_| {
            Status::invalid_argument(format!("height {} does not fit in u32", req.height))
        })?;
        let key = ack_commitment_path(req.client_id.as_bytes(), req.sequence);

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
        tracing::info!(
            ack_bytes = acknowledgement.len(),
            proof_bytes = proof.len(),
            "served acknowledgement"
        );

        Ok(Response::new(QueryAcknowledgementResponse {
            acknowledgement,
            proof,
            proof_height: req.height,
        }))
    }

    #[tracing::instrument(skip(self, _request), name = "grpc.query_next_seq_recv")]
    async fn query_next_seq_recv(
        &self,
        _request: Request<QueryNextSeqRecvRequest>,
    ) -> Result<Response<QueryNextSeqRecvResponse>, Status> {
        tracing::debug!("gRPC QueryNextSeqRecv (unimplemented in v2)");
        Err(Status::unimplemented(
            "QueryNextSeqRecv: nextSequenceSend path was removed in IBC v2",
        ))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.query_ibc_header")]
    async fn query_ibc_header(
        &self,
        request: Request<QueryIbcHeaderRequest>,
    ) -> Result<Response<QueryIbcHeaderResponse>, Status> {
        use prost::Message as _;
        use soroban_client::xdr::{
            LedgerHeader, Limits, PublicKey, ReadXdr, StellarValueExt, WriteXdr,
        };

        let seq = request.into_inner().height as u32;
        tracing::debug!(sequence = seq, "gRPC QueryIbcHeader");

        let ledger = self.api.get_ledger(seq).await.map_err(|error| {
            tracing::error!(%error, sequence = seq, "get_ledger failed");
            Status::internal(format!("getLedger failed: {error}"))
        })?;

        let header = LedgerHeader::from_xdr(&ledger.header_xdr, Limits::none())
            .map_err(|e| Status::internal(format!("LedgerHeader XDR decode: {e}")))?;

        let scp_value = header.scp_value;
        let (scp_node_id, scp_signature) = match &scp_value.ext {
            StellarValueExt::Signed(sig) => {
                let PublicKey::PublicKeyTypeEd25519(pubkey) = &sig.node_id.0;
                (pubkey.0.to_vec(), sig.signature.to_vec())
            }
            StellarValueExt::Basic => (vec![], vec![]),
        };

        let mut basic_scp_value = scp_value.clone();
        basic_scp_value.ext = StellarValueExt::Basic;
        let signed_value_xdr = basic_scp_value
            .to_xdr(Limits::none())
            .map_err(|e| Status::internal(format!("basic StellarValue XDR encode: {e}")))?;

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
            signed_value_xdr,
        };

        let mut header_bytes = vec![];
        stellar_header
            .encode(&mut header_bytes)
            .map_err(|e| Status::internal(format!("StellarHeader encode: {e}")))?;

        Ok(Response::new(QueryIbcHeaderResponse {
            header: header_bytes,
        }))
    }

    #[tracing::instrument(skip(self, request), name = "grpc.events")]
    async fn events(
        &self,
        request: Request<EventsRequest>,
    ) -> Result<Response<EventsResponse>, Status> {
        let contract_id = match self
            .ibc_contract_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .cloned()
        {
            Some(id) => id,
            None => {
                static WARNED_UNCONFIGURED: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !WARNED_UNCONFIGURED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    tracing::warn!(
                        "Events: ROUTER_CONTRACT_ADDRESS is not configured; returning empty event pages. \
                         Deploy the router (`make -C ci deploy-contracts`) then restart the gateway."
                    );
                }
                let latest = self.api.get_latest_ledger().await.map_err(|error| {
                    tracing::error!(%error, "get_latest_ledger failed (events empty-page fallback)");
                    Status::internal(format!("get_latest_ledger failed: {error}"))
                })?;
                return Ok(Response::new(EventsResponse {
                    latest_ledger: latest.into(),
                    cursor: String::new(),
                    events: Vec::new(),
                }));
            }
        };

        let req = request.into_inner();

        let cursor = if !req.cursor.is_empty() {
            EventCursor::Cursor(req.cursor.clone())
        } else if req.start_ledger > 0 {
            EventCursor::StartLedger(req.start_ledger)
        } else {
            return Err(Status::invalid_argument(
                "events: must set either start_ledger or cursor",
            ));
        };

        let limit = if req.limit == 0 {
            None
        } else {
            Some(req.limit)
        };

        let page = self
            .api
            .get_events(&contract_id, cursor, limit)
            .await
            .map_err(|error| {
                tracing::error!(%error, %contract_id, "get_events failed");
                Status::internal(format!("getEvents failed: {error}"))
            })?;

        if page.events.is_empty() {
            tracing::debug!(
                latest_ledger = page.latest_ledger,
                %contract_id,
                "events poll: none"
            );
        } else {
            tracing::info!(
                events = page.events.len(),
                latest_ledger = page.latest_ledger,
                %contract_id,
                "events poll: found router event(s)"
            );
        }

        let events = page
            .events
            .into_iter()
            .map(|ev| {
                let attributes =
                    crate::event_decode::event_attributes(&ev.topics_xdr, &ev.value_xdr)
                        .unwrap_or_default();

                GatewayContractEvent {
                    id: ev.id,
                    ledger: ev.ledger.into(),
                    ledger_closed_at: ev.ledger_closed_at,
                    contract_id: ev.contract_id,
                    tx_hash: ev.tx_hash,
                    topics_xdr: ev.topics_xdr,
                    value_xdr: ev.value_xdr,
                    attributes,
                }
            })
            .collect();

        Ok(Response::new(EventsResponse {
            latest_ledger: page.latest_ledger.into(),
            cursor: page.cursor,
            events,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_commitment_path_layout_matches_ics24() {
        let key = packet_commitment_path(b"10-stellar-0", 0x1234);
        assert_eq!(&key[..12], b"10-stellar-0");
        assert_eq!(key[12], 0x01);
        assert_eq!(&key[13..], &0x1234u64.to_be_bytes());
    }

    #[test]
    fn packet_receipt_and_ack_use_v2_discriminators() {
        assert_eq!(packet_receipt_path(b"c", 0)[1], 0x02);
        assert_eq!(ack_commitment_path(b"c", 0)[1], 0x03);
    }
}
