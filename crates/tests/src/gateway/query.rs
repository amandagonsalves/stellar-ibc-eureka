use ibc::primitives::proto::{Any, Protobuf};
use serde_json::json;
use soroban_client::xdr::{LedgerHeader, Limits, ScVal, WriteXdr};
use tonic::Code;

use stellar_hermes_gateway::proto::{
    EventsRequest, LatestHeightRequest, QueryAcknowledgementRequest, QueryClientStateRequest,
    QueryClientStatesRequest, QueryConsensusStateRequest, QueryIbcHeaderRequest,
    QueryNextSeqRecvRequest, QueryPacketCommitmentRequest, QueryPacketReceiptRequest,
};
use stellar_ibc_core::commitment::{
    ack_commitment_path, packet_commitment_path, packet_receipt_path,
};
use stellar_ibc_core::conversion::{
    scval_bytes, scval_height, scval_string, scval_struct, scval_symbol, scval_to_xdr, scval_vec,
};
use stellar_ibc_core::ibc::client_state::AnyClientState;

use super::mock::{ledger_meta_with_write, GatewayTest};

fn sample_client_state_xdr() -> Vec<u8> {
    let trust_level = scval_struct(vec![
        ("numerator", ScVal::U32(1)),
        ("denominator", ScVal::U32(3)),
    ])
    .unwrap();
    let client_state = scval_struct(vec![
        ("chain_id", scval_string("testchain-1").unwrap()),
        ("trust_level", trust_level),
        ("trusting_period_secs", ScVal::U64(1_209_600)),
        ("unbonding_period_secs", ScVal::U64(1_814_400)),
        ("max_clock_drift_secs", ScVal::U64(40)),
        ("latest_height", scval_height(0, 10).unwrap()),
        ("frozen_height", scval_height(0, 0).unwrap()),
    ])
    .unwrap();
    scval_to_xdr(&client_state).unwrap()
}

#[tokio::test]
async fn latest_height_returns_api_sequence() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.latest_ledger = 777);

    let resp = t
        .query()
        .latest_height(LatestHeightRequest {})
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.revision_height, 777);
    assert_eq!(resp.revision_number, 0);
}

#[tokio::test]
async fn query_next_seq_recv_is_unimplemented_in_v2() {
    let t = GatewayTest::start(None).await;
    let err = t
        .query()
        .query_next_seq_recv(QueryNextSeqRecvRequest {
            client_id: "07-tendermint-0".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::Unimplemented);
}

#[tokio::test]
async fn events_are_decoded_into_attributes() {
    let t = GatewayTest::start(Some([1u8; 32])).await;

    let topic = hex::encode(scval_to_xdr(&scval_symbol("create_client").unwrap()).unwrap());
    let value = hex::encode(
        scval_to_xdr(
            &scval_struct(vec![(
                "client_id",
                scval_string("07-tendermint-0").unwrap(),
            )])
            .unwrap(),
        )
        .unwrap(),
    );

    t.with_data(|d| {
        d.latest_ledger = 5;
        d.events.push(json!({
            "id": "e1",
            "ledger": 5,
            "ledger_closed_at": "2026-01-01T00:00:00Z",
            "contract_id": "Crouter",
            "tx_hash": "deadbeef",
            "topics_xdr": [topic],
            "value_xdr": value,
        }));
    });

    let resp = t
        .query()
        .events(EventsRequest {
            start_ledger: 1,
            cursor: String::new(),
            limit: 0,
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.events.len(), 1);
    let attributes = &resp.events[0].attributes;
    assert!(
        attributes.contains("type=create_client"),
        "got: {attributes}"
    );
    assert!(
        attributes.contains("client_id=07-tendermint-0"),
        "got: {attributes}"
    );
}

#[tokio::test]
async fn query_client_state_converts_soroban_to_protobuf() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| {
        d.client_states
            .insert("07-tendermint-0".into(), sample_client_state_xdr());
    });

    let resp = t
        .query()
        .query_client_state(QueryClientStateRequest {
            client_id: "07-tendermint-0".into(),
            height: 10,
        })
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.client_state.is_empty());
    assert_eq!(resp.proof_height, 10);

    let decoded = <AnyClientState as Protobuf<Any>>::decode_vec(&resp.client_state).unwrap();
    assert_eq!(decoded.chain_id(), "testchain-1");
    assert_eq!(decoded.latest_height(), 10);
}

#[tokio::test]
async fn query_client_state_errors_when_state_missing() {
    let t = GatewayTest::start(None).await;

    let err = t
        .query()
        .query_client_state(QueryClientStateRequest {
            client_id: "absent".into(),
            height: 1,
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::Internal);
}

#[tokio::test]
async fn events_decode_packet_attributes() {
    let t = GatewayTest::start(Some([1u8; 32])).await;

    let payload = scval_struct(vec![
        ("source_port", scval_string("transfer").unwrap()),
        ("dest_port", scval_string("transfer").unwrap()),
    ])
    .unwrap();
    let packet = scval_struct(vec![
        ("sequence", ScVal::U64(7)),
        ("source_client", scval_string("07-tendermint-0").unwrap()),
        ("dest_client", scval_string("08-wasm-0").unwrap()),
        ("payloads", scval_vec(vec![payload]).unwrap()),
    ])
    .unwrap();
    let value =
        hex::encode(scval_to_xdr(&scval_struct(vec![("packet", packet)]).unwrap()).unwrap());
    let topic = hex::encode(scval_to_xdr(&scval_symbol("send_packet").unwrap()).unwrap());

    t.with_data(|d| {
        d.latest_ledger = 5;
        d.events.push(json!({
            "id": "e1",
            "ledger": 5,
            "ledger_closed_at": "2026-01-01T00:00:00Z",
            "contract_id": "Crouter",
            "tx_hash": "deadbeef",
            "topics_xdr": [topic],
            "value_xdr": value,
        }));
    });

    let resp = t
        .query()
        .events(EventsRequest {
            start_ledger: 1,
            cursor: String::new(),
            limit: 0,
        })
        .await
        .unwrap()
        .into_inner();

    let attributes = &resp.events[0].attributes;
    assert!(attributes.contains("type=send_packet"), "got: {attributes}");
    assert!(
        attributes.contains("packet_sequence=7"),
        "got: {attributes}"
    );
    assert!(
        attributes.contains("packet_src_channel=07-tendermint-0"),
        "got: {attributes}"
    );
    assert!(
        attributes.contains("packet_dst_channel=08-wasm-0"),
        "got: {attributes}"
    );
    assert!(
        attributes.contains("packet_src_port=transfer"),
        "got: {attributes}"
    );
    assert!(
        attributes.contains("packet_dst_port=transfer"),
        "got: {attributes}"
    );
}

#[tokio::test]
async fn query_consensus_state_converts_soroban_to_protobuf() {
    let t = GatewayTest::start(None).await;

    let consensus_state = scval_struct(vec![
        ("timestamp_secs", ScVal::U64(1_700_000_000)),
        ("next_validators_hash", scval_bytes(&[7u8; 32]).unwrap()),
        ("root", scval_bytes(&[9u8; 32]).unwrap()),
    ])
    .unwrap();
    let xdr = scval_to_xdr(&consensus_state).unwrap();

    t.with_data(|d| {
        d.consensus_states.insert("07-tendermint-0".into(), xdr);
    });

    let resp = t
        .query()
        .query_consensus_state(QueryConsensusStateRequest {
            client_id: "07-tendermint-0".into(),
            revision_number: 0,
            revision_height: 10,
        })
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.consensus_state.is_empty());
    assert_eq!(resp.proof_height, 10);
}

#[tokio::test]
async fn packet_commitment_absent_yields_nonmembership_proof() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| {
        d.ledgers.insert(5, (vec![], None));
    });

    let resp = t
        .query()
        .query_packet_commitment(QueryPacketCommitmentRequest {
            client_id: "07-tendermint-0".into(),
            sequence: 1,
            height: 5,
        })
        .await
        .unwrap()
        .into_inner();

    assert!(resp.commitment.is_empty());
    assert!(!resp.proof.is_empty());
    assert_eq!(resp.proof_height, 5);
}

#[tokio::test]
async fn state_reflects_contract_write_as_membership_proof() {
    let contract = [7u8; 32];
    let client_id = "07-tendermint-0";
    let key = packet_commitment_path(client_id.as_bytes(), 1);
    let value = vec![0xCDu8; 32];
    let meta = ledger_meta_with_write(contract, key, value);

    let t = GatewayTest::start(Some(contract)).await;
    t.with_data(|d| {
        d.ledgers.insert(5, (vec![], Some(meta)));
    });

    let found = t
        .query()
        .query_packet_commitment(QueryPacketCommitmentRequest {
            client_id: client_id.into(),
            sequence: 1,
            height: 5,
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(found.commitment.len(), 32);
    assert!(!found.proof.is_empty());

    let other = t
        .query()
        .query_packet_commitment(QueryPacketCommitmentRequest {
            client_id: client_id.into(),
            sequence: 2,
            height: 5,
        })
        .await
        .unwrap()
        .into_inner();

    assert!(other.commitment.is_empty());
}

#[tokio::test]
async fn query_ibc_header_encodes_stellar_header() {
    let t = GatewayTest::start(None).await;
    let header_xdr = LedgerHeader::default().to_xdr(Limits::none()).unwrap();
    t.with_data(|d| {
        d.ledgers.insert(5, (header_xdr, None));
    });

    let resp = t
        .query()
        .query_ibc_header(QueryIbcHeaderRequest { height: 5 })
        .await
        .unwrap()
        .into_inner();

    assert!(!resp.header.is_empty());
}

#[tokio::test]
async fn packet_receipt_reflects_contract_write() {
    let contract = [8u8; 32];
    let client_id = "07-tendermint-0";
    let key = packet_receipt_path(client_id.as_bytes(), 1);
    let meta = ledger_meta_with_write(contract, key, vec![0x01]);

    let t = GatewayTest::start(Some(contract)).await;
    t.with_data(|d| {
        d.ledgers.insert(5, (vec![], Some(meta)));
    });

    let resp = t
        .query()
        .query_packet_receipt(QueryPacketReceiptRequest {
            client_id: client_id.into(),
            sequence: 1,
            height: 5,
        })
        .await
        .unwrap()
        .into_inner();

    assert!(resp.received);
    assert!(!resp.proof.is_empty());
}

#[tokio::test]
async fn acknowledgement_reflects_contract_write() {
    let contract = [9u8; 32];
    let client_id = "07-tendermint-0";
    let key = ack_commitment_path(client_id.as_bytes(), 1);
    let meta = ledger_meta_with_write(contract, key, vec![0xEEu8; 32]);

    let t = GatewayTest::start(Some(contract)).await;
    t.with_data(|d| {
        d.ledgers.insert(5, (vec![], Some(meta)));
    });

    let resp = t
        .query()
        .query_acknowledgement(QueryAcknowledgementRequest {
            client_id: client_id.into(),
            sequence: 1,
            height: 5,
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.acknowledgement.len(), 32);
    assert!(!resp.proof.is_empty());
}

#[tokio::test]
async fn query_client_states_lists_decoded_states() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| {
        d.client_states
            .insert("07-tendermint-0".into(), sample_client_state_xdr());
    });

    let resp = t
        .query()
        .query_client_states(QueryClientStatesRequest {})
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.client_states.len(), 1);
    assert_eq!(resp.client_states[0].client_id, "07-tendermint-0");
    assert!(!resp.client_states[0].client_state.is_empty());
}
