use std::collections::HashMap;

use soroban_client::xdr::{
    LedgerCloseMeta, LedgerEntryChange, LedgerEntryData, LedgerKey, Limits, ReadXdr, ScAddress,
    TransactionMeta, WriteXdr,
};
use stellar_hermes_core::rpc::RpcClient;
use stellar_ibc::smt::Smt;

pub struct StateTracker {
    rpc: RpcClient,
    roots: HashMap<u32, [u8; 32]>,
    ibc_contract_id: Option<[u8; 32]>,
    smt: Smt,
}

impl StateTracker {
    pub fn new(rpc: RpcClient, ibc_contract_id: Option<[u8; 32]>) -> Self {
        Self {
            rpc,
            roots: HashMap::new(),
            ibc_contract_id,
            smt: Smt::new(),
        }
    }

    pub async fn root_at(&mut self, seq: u32) -> anyhow::Result<[u8; 32]> {
        if let Some(&root) = self.roots.get(&seq) {
            return Ok(root);
        }
        self.process(seq).await
    }

    async fn process(&mut self, seq: u32) -> anyhow::Result<[u8; 32]> {
        let ledger = self.rpc.get_ledger(seq).await?;

        if let Some(meta_xdr) = ledger.metadata_xdr {
            let meta = LedgerCloseMeta::from_xdr(&meta_xdr, Limits::none())
                .map_err(|e| anyhow::anyhow!("LedgerCloseMeta XDR decode: {e}"))?;

            for change in ledger_changes(&meta) {
                self.apply(change);
            }
        }

        let root = self.smt.root();
        self.roots.insert(seq, root);
        Ok(root)
    }

    fn apply(&mut self, change: LedgerEntryChange) {
        match change {
            LedgerEntryChange::Created(e) | LedgerEntryChange::Updated(e) => {
                if let LedgerEntryData::ContractData(d) = e.data {
                    if !self.matches(&d.contract) {
                        return;
                    }
                    let Ok(k) = d.key.to_xdr(Limits::none()) else {
                        return;
                    };
                    let Ok(v) = d.val.to_xdr(Limits::none()) else {
                        return;
                    };
                    self.smt.insert(&k, &v);
                }
            }
            LedgerEntryChange::Removed(key) => {
                if let LedgerKey::ContractData(d) = key {
                    if !self.matches(&d.contract) {
                        return;
                    }
                    let Ok(k) = d.key.to_xdr(Limits::none()) else {
                        return;
                    };
                    self.smt.remove(&k);
                }
            }
            _ => {}
        }
    }

    fn matches(&self, addr: &ScAddress) -> bool {
        let ScAddress::Contract(hash) = addr else {
            return false;
        };
        match &self.ibc_contract_id {
            None => true,
            Some(id) => &hash.0 == id,
        }
    }
}

fn ledger_changes(meta: &LedgerCloseMeta) -> Vec<LedgerEntryChange> {
    let mut out = vec![];
    match meta {
        LedgerCloseMeta::V0(v) => {
            for tx in v.tx_processing.iter() {
                collect_tx_changes(&tx.tx_apply_processing, &mut out);
            }
        }
        LedgerCloseMeta::V1(v) => {
            for tx in v.tx_processing.iter() {
                collect_tx_changes(&tx.tx_apply_processing, &mut out);
            }
        }
        LedgerCloseMeta::V2(v) => {
            for tx in v.tx_processing.iter() {
                collect_tx_changes(&tx.tx_apply_processing, &mut out);
            }
        }
    }
    out
}

fn collect_tx_changes(meta: &TransactionMeta, out: &mut Vec<LedgerEntryChange>) {
    match meta {
        TransactionMeta::V0(changes) => {
            out.extend(changes.iter().cloned());
        }
        TransactionMeta::V1(v) => {
            out.extend(v.tx_changes.iter().cloned());
            for op in v.operations.iter() {
                out.extend(op.changes.iter().cloned());
            }
        }
        TransactionMeta::V2(v) => {
            out.extend(v.tx_changes_before.iter().cloned());
            for op in v.operations.iter() {
                out.extend(op.changes.iter().cloned());
            }
            out.extend(v.tx_changes_after.iter().cloned());
        }
        TransactionMeta::V3(v) => {
            out.extend(v.tx_changes_before.iter().cloned());
            for op in v.operations.iter() {
                out.extend(op.changes.iter().cloned());
            }
            out.extend(v.tx_changes_after.iter().cloned());
        }
        #[allow(unreachable_patterns)]
        _ => {}
    }
}
