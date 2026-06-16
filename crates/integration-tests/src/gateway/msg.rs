use soroban_client::xdr::ScVal;
use tonic::Code;

use stellar_hermes_gateway::proto::{
    MsgAckPacketRequest, MsgCreateClientRequest, MsgRecvPacketRequest,
    MsgRegisterCounterpartyRequest, MsgSubmitMisbehaviourRequest, MsgTimeoutPacketRequest,
    MsgUpdateClientRequest, SubmitSignedTxRequest,
};
use stellar_ibc_core::conversion::{
    scval_as_bytes, scval_bytes, scval_from_xdr, scval_string, scval_to_xdr, scval_vec_of_bytes,
};
use stellar_ibc_core::ibc::client_state::AnyClientState;
use stellar_ibc_core::ibc::consensus_state::AnyConsensusState;

use super::mock::{
    sample_packet, tm_client_state_protobuf, tm_consensus_state_protobuf, GatewayTest,
    FIXTURE_CHAIN_ID, FIXTURE_LATEST_HEIGHT,
};

#[tokio::test]
async fn create_client_rejects_empty_client_type() {
    let t = GatewayTest::start(None).await;
    let err = t
        .msg()
        .create_client(MsgCreateClientRequest {
            client_state: vec![],
            consensus_state: vec![],
            client_type: String::new(),
            height: 0,
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("client_type"));
}

#[tokio::test]
async fn update_client_rejects_empty_client_id() {
    let t = GatewayTest::start(None).await;
    let err = t
        .msg()
        .update_client(MsgUpdateClientRequest {
            client_id: String::new(),
            header: vec![],
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("client_id"));
}

#[tokio::test]
async fn register_counterparty_rejects_empty_ids() {
    let t = GatewayTest::start(None).await;
    let err = t
        .msg()
        .register_counterparty(MsgRegisterCounterpartyRequest {
            client_id: String::new(),
            counterparty_client_id: String::new(),
            counterparty_commitment_prefix: vec![],
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::InvalidArgument);
}

#[tokio::test]
async fn submit_misbehaviour_rejects_empty_client_id() {
    let t = GatewayTest::start(None).await;
    let err = t
        .msg()
        .submit_misbehaviour(MsgSubmitMisbehaviourRequest {
            client_id: String::new(),
            client_message: vec![1, 2, 3],
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::InvalidArgument);
    assert!(err.message().contains("client_id"));
}

#[tokio::test]
async fn recv_packet_forwards_packet_proof_and_height() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-recv".to_vec());

    let packet = sample_packet();
    let resp = t
        .msg()
        .recv_packet(MsgRecvPacketRequest {
            packet: scval_to_xdr(&packet).unwrap(),
            proof: vec![9, 9, 9],
            proof_height: 42,
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-recv");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "recv_packet");
        assert_eq!(call.signer, "GSIGNER");
        assert_eq!(call.args.len(), 3);
        assert_eq!(scval_from_xdr(&call.args[0]).unwrap(), packet);
        assert_eq!(scval_from_xdr(&call.args[2]).unwrap(), ScVal::U64(42));
    });
}

#[tokio::test]
async fn ack_packet_wraps_acknowledgement_as_vec_of_bytes() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-ack".to_vec());

    let packet = sample_packet();
    let ack = vec![0xAA, 0xBB];
    let resp = t
        .msg()
        .ack_packet(MsgAckPacketRequest {
            packet: scval_to_xdr(&packet).unwrap(),
            acknowledgement: ack.clone(),
            proof: vec![1],
            proof_height: 7,
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-ack");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "acknowledge_packet");
        assert_eq!(call.args.len(), 4);
        assert_eq!(scval_from_xdr(&call.args[0]).unwrap(), packet);
        assert_eq!(
            scval_from_xdr(&call.args[1]).unwrap(),
            scval_vec_of_bytes(&[ack]).unwrap()
        );
        assert_eq!(scval_from_xdr(&call.args[3]).unwrap(), ScVal::U64(7));
    });
}

#[tokio::test]
async fn timeout_packet_forwards_packet_and_height() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-timeout".to_vec());

    let packet = sample_packet();
    let resp = t
        .msg()
        .timeout_packet(MsgTimeoutPacketRequest {
            packet: scval_to_xdr(&packet).unwrap(),
            proof: vec![5],
            proof_height: 99,
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-timeout");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "timeout_packet");
        assert_eq!(call.args.len(), 3);
        assert_eq!(scval_from_xdr(&call.args[0]).unwrap(), packet);
        assert_eq!(scval_from_xdr(&call.args[2]).unwrap(), ScVal::U64(99));
    });
}

#[tokio::test]
async fn submit_signed_tx_returns_hash_from_api() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.submit_hash = "abc123".to_string());

    let resp = t
        .msg()
        .submit_signed_tx(SubmitSignedTxRequest {
            tx_xdr: b"signed-bytes".to_vec(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_hash, "abc123");

    t.with_data(|d| {
        assert_eq!(
            d.submit_calls.last().unwrap(),
            &hex::encode(b"signed-bytes")
        );
    });
}

#[tokio::test]
async fn create_client_converts_client_and_consensus_to_soroban() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-create".to_vec());

    let resp = t
        .msg()
        .create_client(MsgCreateClientRequest {
            client_state: tm_client_state_protobuf(),
            consensus_state: tm_consensus_state_protobuf(),
            client_type: "07-tendermint".into(),
            height: 0,
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-create");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "create_client");
        assert_eq!(call.args.len(), 4);
        assert_eq!(
            scval_from_xdr(&call.args[0]).unwrap(),
            scval_string("07-tendermint").unwrap()
        );

        let cs_xdr = scval_as_bytes(&scval_from_xdr(&call.args[1]).unwrap()).unwrap();
        let cs = AnyClientState::from_soroban_xdr(&cs_xdr).unwrap();
        assert_eq!(cs.chain_id(), FIXTURE_CHAIN_ID);
        assert_eq!(cs.latest_height(), FIXTURE_LATEST_HEIGHT);

        let cons_xdr = scval_as_bytes(&scval_from_xdr(&call.args[2]).unwrap()).unwrap();
        AnyConsensusState::from_soroban_xdr(&cons_xdr).expect("consensus decodes");

        assert_eq!(
            scval_from_xdr(&call.args[3]).unwrap(),
            ScVal::U64(FIXTURE_LATEST_HEIGHT)
        );
    });
}

#[tokio::test]
async fn update_client_passes_non_tendermint_header_through() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-update".to_vec());

    let resp = t
        .msg()
        .update_client(MsgUpdateClientRequest {
            client_id: "08-wasm-0".into(),
            header: b"raw-header".to_vec(),
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-update");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "update_client");
        assert_eq!(call.args.len(), 2);
        assert_eq!(
            scval_from_xdr(&call.args[0]).unwrap(),
            scval_string("08-wasm-0").unwrap()
        );
        assert_eq!(
            scval_from_xdr(&call.args[1]).unwrap(),
            scval_bytes(b"raw-header").unwrap()
        );
    });
}

#[tokio::test]
async fn register_counterparty_forwards_ids_and_prefix() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-register".to_vec());

    let resp = t
        .msg()
        .register_counterparty(MsgRegisterCounterpartyRequest {
            client_id: "07-tendermint-0".into(),
            counterparty_client_id: "08-wasm-0".into(),
            counterparty_commitment_prefix: vec![b"ibc".to_vec()],
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-register");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "register_counterparty");
        assert_eq!(call.args.len(), 3);
        assert_eq!(
            scval_from_xdr(&call.args[0]).unwrap(),
            scval_string("07-tendermint-0").unwrap()
        );
        assert_eq!(
            scval_from_xdr(&call.args[1]).unwrap(),
            scval_string("08-wasm-0").unwrap()
        );
        assert_eq!(
            scval_from_xdr(&call.args[2]).unwrap(),
            scval_vec_of_bytes(&[b"ibc".to_vec()]).unwrap()
        );
    });
}

#[tokio::test]
async fn submit_misbehaviour_forwards_client_message() {
    let t = GatewayTest::start(None).await;
    t.with_data(|d| d.prepare_tx_xdr = b"unsigned-misbehave".to_vec());

    let resp = t
        .msg()
        .submit_misbehaviour(MsgSubmitMisbehaviourRequest {
            client_id: "07-tendermint-0".into(),
            client_message: b"evidence".to_vec(),
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_xdr, b"unsigned-misbehave");

    t.with_data(|d| {
        let call = d.prepare_calls.last().expect("prepare called");
        assert_eq!(call.method, "update_client");
        assert_eq!(call.args.len(), 2);
        assert_eq!(
            scval_from_xdr(&call.args[0]).unwrap(),
            scval_string("07-tendermint-0").unwrap()
        );
        assert_eq!(
            scval_from_xdr(&call.args[1]).unwrap(),
            scval_bytes(b"evidence").unwrap()
        );
    });
}

#[tokio::test]
async fn submit_signed_tx_returns_decoded_return_value() {
    let t = GatewayTest::start(None).await;
    let return_value = scval_to_xdr(&ScVal::U64(99)).unwrap();
    t.with_data(|d| {
        d.submit_hash = "h1".to_string();
        d.submit_return_value_xdr = return_value.clone();
    });

    let resp = t
        .msg()
        .submit_signed_tx(SubmitSignedTxRequest {
            tx_xdr: b"signed".to_vec(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(resp.tx_hash, "h1");
    assert_eq!(resp.return_value, return_value);
}

#[tokio::test]
async fn recv_packet_rejects_invalid_packet_xdr() {
    let t = GatewayTest::start(None).await;
    let err = t
        .msg()
        .recv_packet(MsgRecvPacketRequest {
            packet: vec![0xFF, 0xFF, 0xFF, 0xFF],
            proof: vec![],
            proof_height: 1,
            signer: "GSIGNER".into(),
        })
        .await
        .unwrap_err();

    assert_eq!(err.code(), Code::InvalidArgument);
}
