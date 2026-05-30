use std::sync::Arc;
use std::time::Duration;

use soroban_client::{
    contract::{ContractBehavior, Contracts},
    soroban_rpc::{EventType, TransactionStatus},
    transaction::TransactionBehavior,
    transaction_builder::{TransactionBuilder, TransactionBuilderBehavior},
    xdr::{LedgerKey, Limits, ReadXdr, ScVal, TransactionEnvelope, WriteXdr},
    EventFilter, Options, Pagination, Server,
};
use stellar_ibc_core::types::{LedgerData, SubmittedTx};

#[derive(Clone, Debug)]
pub struct EventRecord {
    pub id: String,
    pub ledger: u32,
    pub ledger_closed_at: String,
    pub contract_id: String,
    pub tx_hash: String,
    pub topics_xdr: Vec<Vec<u8>>,
    pub value_xdr: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct EventsPage {
    pub latest_ledger: u32,
    pub cursor: String,
    pub events: Vec<EventRecord>,
}

#[derive(Clone, Debug)]
pub enum EventCursor {
    Cursor(String),
    StartLedger(u32),
}

#[derive(Clone)]
pub struct RpcClient {
    pub server: Arc<Server>,
    rpc_url: String,
    http: reqwest::Client,
    signer: String,
}

impl RpcClient {
    pub fn new(rpc_url: &str, signer: &str) -> anyhow::Result<Self> {
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
            signer: String::from(signer),
        })
    }

    pub async fn get_latest_ledger(&self) -> anyhow::Result<u32> {
        let info = self.server.get_latest_ledger().await?;

        Ok(info.sequence)
    }

    pub async fn get_ledger(&self, sequence: u32) -> anyhow::Result<LedgerData> {
        let resp = self
            .server
            .get_ledgers(Pagination::From(sequence), Some(1))
            .await
            .map_err(|e| anyhow::anyhow!("getLedgers RPC failed: {e}"))?;

        let info = resp
            .ledgers
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("getLedgers: no ledger at sequence {sequence}"))?;

        if info.sequence != sequence {
            return Err(anyhow::anyhow!(
                "getLedgers: expected seq {sequence}, got {}",
                info.sequence
            ));
        }

        let header_xdr = info
            .to_header()
            .ok_or_else(|| anyhow::anyhow!("getLedgers: no headerXdr for sequence {sequence}"))?
            .header
            .to_xdr(Limits::none())
            .map_err(|e| anyhow::anyhow!("LedgerHeader XDR encode failed: {e}"))?;

        let metadata_xdr = info
            .to_metadata()
            .map(|m| {
                m.to_xdr(Limits::none())
                    .map_err(|e| anyhow::anyhow!("LedgerCloseMeta XDR encode failed: {e}"))
            })
            .transpose()?;

        Ok(LedgerData {
            sequence,
            header_xdr,
            metadata_xdr,
        })
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

    pub async fn get_events(
        &self,
        contract_id: &str,
        cursor: EventCursor,
        limit: Option<u32>,
    ) -> anyhow::Result<EventsPage> {
        let pagination = match cursor {
            EventCursor::Cursor(c) => Pagination::Cursor(c),
            EventCursor::StartLedger(s) => Pagination::From(s),
        };
        let filter = EventFilter::new(EventType::Contract).contract(contract_id);

        let resp = self
            .server
            .get_events(pagination, vec![filter], limit)
            .await
            .map_err(|e| anyhow::anyhow!("getEvents RPC failed: {e}"))?;

        let mut events = Vec::with_capacity(resp.events.len());
        for ev in &resp.events {
            let topics_xdr = ev
                .topic()
                .into_iter()
                .map(|t| {
                    t.to_xdr(Limits::none())
                        .map_err(|e| anyhow::anyhow!("event topic XDR encode: {e}"))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let value_xdr = ev
                .value()
                .to_xdr(Limits::none())
                .map_err(|e| anyhow::anyhow!("event value XDR encode: {e}"))?;
            events.push(EventRecord {
                id: ev.id.clone(),
                ledger: ev.ledger as u32,
                ledger_closed_at: ev.ledger_closed_at.clone(),
                contract_id: ev.contract_id.clone(),
                tx_hash: ev.tx_hash.clone(),
                topics_xdr,
                value_xdr,
            });
        }

        Ok(EventsPage {
            latest_ledger: resp.latest_ledger as u32,
            cursor: resp.cursor.unwrap_or_default(),
            events,
        })
    }

    pub async fn build_unsigned_tx(
        &self,
        contract_id: &str,
        method: &str,
        args_xdr: &[Vec<u8>],
        network_passphrase: &str,
        base_fee: u32,
    ) -> anyhow::Result<Vec<u8>> {
        let mut args = Vec::with_capacity(args_xdr.len());
        for (i, raw) in args_xdr.iter().enumerate() {
            let sc = ScVal::from_xdr(raw, Limits::none())
                .map_err(|e| anyhow::anyhow!("arg {i} ScVal XDR decode: {e}"))?;
            args.push(sc);
        }

        let contract = Contracts::new(contract_id)
            .map_err(|e| anyhow::anyhow!("invalid contract id {contract_id}: {e}"))?;
        let operation = contract.call(method, Some(args));

        let address: &str = &self.signer;

        let mut source = self
            .server
            .get_account(address)
            .await
            .map_err(|e| anyhow::anyhow!("get_account({address}): {e:?}"))?;

        let unsigned = TransactionBuilder::new(&mut source, network_passphrase, None)
            .fee(base_fee)
            .add_operation(operation)
            .build();

        let prepared = self
            .server
            .prepare_transaction(&unsigned)
            .await
            .map_err(|e| anyhow::anyhow!("prepare_transaction: {e:?}"))?;

        let envelope = prepared
            .to_envelope()
            .map_err(|e| anyhow::anyhow!("to_envelope: {e}"))?;
        envelope
            .to_xdr(Limits::none())
            .map_err(|e| anyhow::anyhow!("envelope XDR encode: {e}"))
    }

    pub async fn submit_and_wait(&self, tx_xdr: &[u8]) -> anyhow::Result<SubmittedTx> {
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
            TransactionStatus::Success => {
                let return_value = result.to_result_meta().and_then(|(_, rv)| rv);
                Ok(SubmittedTx { hash, return_value })
            }
            TransactionStatus::Failed => Err(anyhow::anyhow!("transaction {hash} failed on-chain")),
            TransactionStatus::NotFound => {
                Err(anyhow::anyhow!("transaction {hash} not found after 30s"))
            }
        }
    }
}
