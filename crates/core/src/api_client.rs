use soroban_client::xdr::{Limits, ReadXdr, ScVal, WriteXdr};

use crate::types::{LedgerData, SubmittedTx};

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
    http: reqwest::Client,
}

pub struct EventRecord {
    pub id: String,
    pub ledger: u32,
    pub ledger_closed_at: String,
    pub contract_id: String,
    pub tx_hash: String,
    pub topics_xdr: Vec<Vec<u8>>,
    pub value_xdr: Vec<u8>,
}

pub struct EventsPage {
    pub latest_ledger: u32,
    pub cursor: String,
    pub events: Vec<EventRecord>,
}

pub enum EventCursor {
    Cursor(String),
    StartLedger(u32),
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn get_json(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let resp = self
            .http
            .get(self.url(path))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("GET {path} failed: {e}"))?;
        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("GET {path} response parse failed: {e}"))?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("GET {path} returned {status}: {body}"));
        }
        Ok(body)
    }

    async fn post_json(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        let resp = self
            .http
            .post(self.url(path))
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("POST {path} failed: {e}"))?;
        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("POST {path} response parse failed: {e}"))?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("POST {path} returned {status}: {body}"));
        }
        Ok(body)
    }

    pub async fn get_latest_ledger(&self) -> anyhow::Result<u32> {
        let body = self.get_json("/ledger/latest").await?;
        body.get("sequence")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .ok_or_else(|| anyhow::anyhow!("missing 'sequence' in /ledger/latest response"))
    }

    pub async fn get_transfer_balance(&self, denom: &str, address_hex: &str) -> anyhow::Result<i128> {
        let body = self
            .get_json(&format!("/stellar/transfer/balance/{denom}/{address_hex}"))
            .await?;
        body.get("balance")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<i128>().ok())
            .ok_or_else(|| anyhow::anyhow!("missing/invalid 'balance' in transfer balance response"))
    }

    pub async fn list_client_ids(&self) -> anyhow::Result<Vec<String>> {
        let body = self.get_json("/stellar/clients").await?;
        let mut ids = Vec::new();
        if let Some(groups) = body.get("clients").and_then(|c| c.as_array()) {
            for group in groups {
                if let Some(arr) = group.get("client_ids").and_then(|v| v.as_array()) {
                    ids.extend(arr.iter().filter_map(|v| v.as_str().map(str::to_string)));
                }
            }
        }
        Ok(ids)
    }

    pub async fn get_client_state_xdr(&self, client_id: &str) -> anyhow::Result<Vec<u8>> {
        let body = self
            .get_json(&format!("/stellar/clients/{client_id}/state"))
            .await?;
        let hex = body
            .get("client_state_xdr")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'client_state_xdr' in response"))?;
        hex::decode(hex).map_err(|e| anyhow::anyhow!("client_state_xdr hex decode: {e}"))
    }

    pub async fn get_consensus_state_xdr(
        &self,
        client_id: &str,
        height: u64,
    ) -> anyhow::Result<Vec<u8>> {
        let body = self
            .get_json(&format!("/stellar/clients/{client_id}/consensus/{height}"))
            .await?;
        let hex = body
            .get("consensus_state_xdr")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'consensus_state_xdr' in response"))?;
        hex::decode(hex).map_err(|e| anyhow::anyhow!("consensus_state_xdr hex decode: {e}"))
    }

    pub async fn get_ledger(&self, sequence: u32) -> anyhow::Result<LedgerData> {
        let body = self.get_json(&format!("/ledger/{sequence}")).await?;

        let header_hex = body
            .get("header_xdr")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'header_xdr' for ledger {sequence}"))?;
        let header_xdr =
            hex::decode(header_hex).map_err(|e| anyhow::anyhow!("header_xdr hex decode: {e}"))?;

        let metadata_xdr = match body.get("metadata_xdr").and_then(|v| v.as_str()) {
            Some(meta_hex) => Some(
                hex::decode(meta_hex)
                    .map_err(|e| anyhow::anyhow!("metadata_xdr hex decode: {e}"))?,
            ),
            None => None,
        };

        Ok(LedgerData {
            sequence,
            header_xdr,
            metadata_xdr,
        })
    }

    pub async fn get_events(
        &self,
        contract_id: &str,
        cursor: EventCursor,
        limit: Option<u32>,
    ) -> anyhow::Result<EventsPage> {
        let mut path = format!("/events?contract_id={contract_id}");
        match cursor {
            EventCursor::Cursor(c) => path.push_str(&format!("&cursor={c}")),
            EventCursor::StartLedger(s) => path.push_str(&format!("&start_ledger={s}")),
        }
        if let Some(limit) = limit {
            path.push_str(&format!("&limit={limit}"));
        }

        let body = self.get_json(&path).await?;

        let latest_ledger = body
            .get("latest_ledger")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .ok_or_else(|| anyhow::anyhow!("missing 'latest_ledger' in /events response"))?;
        let cursor = body
            .get("cursor")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_owned();

        let raw_events = body
            .get("events")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("missing 'events' array in /events response"))?;

        let mut events = Vec::with_capacity(raw_events.len());
        for ev in raw_events {
            let topics_xdr = ev
                .get("topics_xdr")
                .and_then(|v| v.as_array())
                .map(|topics| {
                    topics
                        .iter()
                        .filter_map(|t| t.as_str())
                        .map(hex::decode)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()
                .map_err(|e| anyhow::anyhow!("topics_xdr hex decode: {e}"))?
                .unwrap_or_default();

            let value_xdr = ev
                .get("value_xdr")
                .and_then(|v| v.as_str())
                .map(hex::decode)
                .transpose()
                .map_err(|e| anyhow::anyhow!("value_xdr hex decode: {e}"))?
                .unwrap_or_default();

            events.push(EventRecord {
                id: string_field(ev, "id"),
                ledger: ev.get("ledger").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                ledger_closed_at: string_field(ev, "ledger_closed_at"),
                contract_id: string_field(ev, "contract_id"),
                tx_hash: string_field(ev, "tx_hash"),
                topics_xdr,
                value_xdr,
            });
        }

        Ok(EventsPage {
            latest_ledger,
            cursor,
            events,
        })
    }

    pub async fn build_unsigned_tx(
        &self,
        signer: &str,
        method: &str,
        args: Vec<ScVal>,
    ) -> anyhow::Result<Vec<u8>> {
        let args_xdr = args
            .iter()
            .map(|arg| {
                arg.to_xdr(Limits::none())
                    .map(hex::encode)
                    .map_err(|e| anyhow::anyhow!("ScVal XDR encode: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let body = serde_json::json!({
            "signer": signer,
            "method": method,
            "args_xdr": args_xdr,
        });
        let resp = self.post_json("/tx/prepare", body).await?;

        let tx_hex = resp
            .get("tx_xdr")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'tx_xdr' in /tx/prepare response"))?;

        hex::decode(tx_hex).map_err(|e| anyhow::anyhow!("tx_xdr hex decode: {e}"))
    }

    pub async fn submit_and_wait(&self, tx_xdr: &[u8]) -> anyhow::Result<SubmittedTx> {
        let body = serde_json::json!({ "tx_xdr": hex::encode(tx_xdr) });
        let resp = self.post_json("/tx/submit", body).await?;
        let hash = resp
            .get("hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("missing 'hash' in /tx/submit response"))?
            .to_owned();
        let return_value = match resp.get("return_value_xdr").and_then(|v| v.as_str()) {
            Some(value_hex) => {
                let bytes = hex::decode(value_hex)
                    .map_err(|e| anyhow::anyhow!("return_value_xdr hex decode: {e}"))?;
                Some(
                    ScVal::from_xdr(&bytes, Limits::none())
                        .map_err(|e| anyhow::anyhow!("return_value ScVal decode: {e}"))?,
                )
            }
            None => None,
        };

        Ok(SubmittedTx { hash, return_value })
    }
}

fn string_field(value: &serde_json::Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned()
}
