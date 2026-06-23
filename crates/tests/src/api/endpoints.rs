use serde_json::json;

use super::mock::{sample_contract_id, ApiTest};

#[tokio::test]
async fn openapi_is_served_with_all_documented_paths() {
    let t = ApiTest::start("", "").await;

    let spec = t.get_json("/api-docs/openapi.json").await;
    let paths = spec["paths"].as_object().expect("openapi paths");

    for path in [
        "/health",
        "/ledger/latest",
        "/ledger/{sequence}",
        "/events",
        "/stellar/transfer/balance/{denom}/{address}",
        "/stellar/clients",
        "/stellar/clients/{client_id}/state",
        "/stellar/clients/{client_id}/consensus/{height}",
        "/tx/prepare",
        "/tx/submit",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path: {path}");
    }
}

#[tokio::test]
async fn health_responds_even_when_rpc_unavailable() {
    let t = ApiTest::start("", "").await;

    let (status, body) = t.get("/health").await;

    assert_eq!(status, 200);
    assert!(body.contains("Stellar IBC API is healthy"), "got: {body}");
}

#[tokio::test]
async fn tx_prepare_without_router_returns_bad_gateway() {
    let t = ApiTest::start("", "").await;

    let (status, body) = t
        .post(
            "/tx/prepare",
            json!({ "signer": "GABC", "method": "noop", "args_xdr": [] }),
        )
        .await;

    assert_eq!(status, 502);
    assert!(body.contains("ROUTER_CONTRACT_ADDRESS"), "got: {body}");
}

#[tokio::test]
async fn tx_prepare_requires_a_signer() {
    let t = ApiTest::start(&sample_contract_id(), "").await;

    let (status, body) = t
        .post(
            "/tx/prepare",
            json!({ "signer": "", "method": "noop", "args_xdr": [] }),
        )
        .await;

    assert_eq!(status, 502);
    assert!(body.contains("signer"), "got: {body}");
}

#[tokio::test]
async fn tx_prepare_rejects_malformed_args_hex() {
    let t = ApiTest::start(&sample_contract_id(), "").await;

    let (status, body) = t
        .post(
            "/tx/prepare",
            json!({ "signer": "GABC", "method": "noop", "args_xdr": ["zzzz"] }),
        )
        .await;

    assert_eq!(status, 400);
    assert!(body.contains("args_xdr"), "got: {body}");
}

#[tokio::test]
async fn tx_submit_rejects_malformed_hex() {
    let t = ApiTest::start("", "").await;

    let (status, body) = t.post("/tx/submit", json!({ "tx_xdr": "zzzz" })).await;

    assert_eq!(status, 400);
    assert!(body.contains("tx_xdr"), "got: {body}");
}

#[tokio::test]
async fn list_clients_without_router_returns_bad_gateway() {
    let t = ApiTest::start("", "").await;

    let (status, body) = t.get("/stellar/clients").await;

    assert_eq!(status, 502);
    assert!(body.contains("ROUTER_CONTRACT_ADDRESS"), "got: {body}");
}

#[tokio::test]
async fn client_state_without_router_returns_bad_gateway() {
    let t = ApiTest::start("", "").await;

    let (status, body) = t.get("/stellar/clients/07-tendermint-0/state").await;

    assert_eq!(status, 502);
    assert!(body.contains("ROUTER_CONTRACT_ADDRESS"), "got: {body}");
}

#[tokio::test]
async fn transfer_balance_without_contract_returns_bad_gateway() {
    let t = ApiTest::start("", "").await;

    let (status, body) = t.get("/stellar/transfer/balance/uatom/abcd").await;

    assert_eq!(status, 502);
    assert!(body.contains("TRANSFER_CONTRACT_ADDRESS"), "got: {body}");
}
