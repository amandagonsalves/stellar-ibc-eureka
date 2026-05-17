use std::sync::Arc;
use std::time::Duration;

use soroban_client::soroban_rpc::TransactionStatus;
use soroban_client::xdr::{Limits, ReadXdr, TransactionEnvelope, WriteXdr};
use soroban_client::{Options, Server};

use soroban_client::xdr::LedgerKey;

#[derive(Clone)]
pub struct RpcClient {
    pub server: Arc<Server>,
    rpc_url: String,
    http: reqwest::Client,
}

impl RpcClient {
    pub fn new(rpc_url: &str) -> anyhow::Result<Self> {
        let server = Server::new(
            rpc_url,
            Options {
                allow_http: true,
                ..Default::default()
            },
        )?;
        Ok(Self {
            server: Arc::new(server),
            rpc_url: rpc_url.to_owned(),
            http: reqwest::Client::new(),
        })
    }

    pub async fn latest_ledger_sequence(&self) -> anyhow::Result<u32> {
        let info = self.server.get_latest_ledger().await?;
        Ok(info.sequence)
    }

    pub async fn get_ledger_entry(&self, key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        let ledger_key = LedgerKey::from_xdr(key, Limits::none())
            .map_err(|e| anyhow::anyhow!("invalid LedgerKey XDR: {e}"))?;

        let resp = self
            .server
            .get_ledger_entries(vec![ledger_key])
            .await
            .map_err(|e| anyhow::anyhow!("getLedgerEntries RPC failed: {e}"))?;

        let entries = match resp.entries {
            Some(e) if !e.is_empty() => e,
            _ => return Ok(None),
        };

        let data_xdr = entries
            .into_iter()
            .next()
            .unwrap()
            .to_data()
            .to_xdr(Limits::none())
            .map_err(|e| anyhow::anyhow!("failed to re-encode LedgerEntryData: {e}"))?;

        Ok(Some(data_xdr))
    }

    pub async fn submit_and_wait(&self, tx_xdr: &[u8]) -> anyhow::Result<String> {
        let envelope = TransactionEnvelope::from_xdr(tx_xdr, Limits::none())
            .map_err(|e| anyhow::anyhow!("invalid TransactionEnvelope XDR: {e}"))?;

        let tx_b64 = envelope
            .to_xdr_base64(Limits::none())
            .map_err(|e| anyhow::anyhow!("failed to base64-encode transaction: {e}"))?;

        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "sendTransaction",
            "params": {"transaction": tx_b64}
        });

        let resp: serde_json::Value = self
            .http
            .post(&self.rpc_url)
            .json(&body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("sendTransaction HTTP failed: {e}"))?
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("sendTransaction response parse failed: {e}"))?;

        let hash = resp
            .pointer("/result/hash")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                let msg = resp
                    .pointer("/error/message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown RPC error");
                anyhow::anyhow!("sendTransaction rejected: {msg}")
            })?
            .to_owned();

        let result = self
            .server
            .wait_transaction(&hash, Duration::from_secs(30))
            .await
            .map_err(|(e, _)| anyhow::anyhow!("wait_transaction failed: {e}"))?;

        match result.status {
            TransactionStatus::Success => Ok(hash),
            TransactionStatus::Failed => {
                Err(anyhow::anyhow!("transaction {hash} failed on-chain"))
            }
            TransactionStatus::NotFound => {
                Err(anyhow::anyhow!("transaction {hash} not found after 30s"))
            }
        }
    }
}