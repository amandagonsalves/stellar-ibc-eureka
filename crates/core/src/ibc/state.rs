use std::collections::HashMap;

use sha2::{Digest, Sha256};
use soroban_client::xdr::{
    LedgerCloseMeta, LedgerEntryChange, LedgerEntryData, LedgerKey, Limits, ReadXdr, ScAddress,
    TransactionMeta,
};

use crate::{
    api_client::ApiClient,
    conversion::{scval_as_bytes, scval_to_v2_path},
    proof::{serialize_membership_proof_with_index, serialize_non_membership_proof_with_index},
    smt::Smt,
};
pub enum PathLookup {
    Found {
        value_hash: [u8; 32],
        proof_bytes: Vec<u8>,
    },
    Absent {
        proof_bytes: Vec<u8>,
    },
}

fn key_index(key: &[u8]) -> u64 {
    let h: [u8; 32] = Sha256::digest(key).into();
    u64::from_be_bytes(h[..8].try_into().expect("sha256 has 32 bytes"))
}

pub struct State {
    api: ApiClient,
    roots: HashMap<u32, [u8; 32]>,
    ibc_contract_id: Option<[u8; 32]>,
    smt: Smt,
    last_processed: Option<u32>,
}

impl State {
    pub fn new(api: ApiClient, ibc_contract_id: Option<[u8; 32]>) -> Self {
        Self {
            api,
            roots: HashMap::new(),
            ibc_contract_id,
            smt: Smt::new(),
            last_processed: None,
        }
    }

    pub async fn root_at(&mut self, seq: u32) -> anyhow::Result<[u8; 32]> {
        if let Some(&root) = self.roots.get(&seq) {
            return Ok(root);
        }
        self.process_through(seq).await
    }

    pub async fn proof_for_path(&mut self, seq: u32, key: &[u8]) -> anyhow::Result<PathLookup> {
        self.root_at(seq).await?;
        let index = key_index(key);
        match self.smt.generate_membership_proof(key) {
            Some(proof) => {
                let value_hash = proof.value_hash;
                let bytes = serialize_membership_proof_with_index(
                    &proof,
                    key,
                    value_hash.as_slice(),
                    index,
                );
                Ok(PathLookup::Found {
                    value_hash,
                    proof_bytes: bytes,
                })
            }
            _ => {
                let proof = self
                    .smt
                    .generate_non_membership_proof(key)
                    .ok_or_else(|| anyhow::anyhow!("non-membership proof unavailable for key"))?;
                let bytes = serialize_non_membership_proof_with_index(&proof, key, index);
                Ok(PathLookup::Absent { proof_bytes: bytes })
            }
        }
    }

    async fn process_through(&mut self, seq: u32) -> anyhow::Result<[u8; 32]> {
        let start = match self.last_processed {
            Some(last) if last >= seq => {
                let root = self.smt.root();
                self.roots.insert(seq, root);
                return Ok(root);
            }
            Some(last) => last + 1,
            _ => seq,
        };

        if start < seq {
            tracing::info!(
                from = start,
                to = seq,
                "replaying ledger range into smt (cumulative)"
            );
        }

        for ledger_seq in start..=seq {
            self.apply_ledger(ledger_seq).await?;
        }

        self.last_processed = Some(seq);
        let root = self.smt.root();
        self.roots.insert(seq, root);
        Ok(root)
    }

    async fn apply_ledger(&mut self, seq: u32) -> anyhow::Result<()> {
        let ledger = self.api.get_ledger(seq).await?;

        let mut changes_applied = 0usize;
        if let Some(meta_xdr) = ledger.metadata_xdr {
            let meta = LedgerCloseMeta::from_xdr(&meta_xdr, Limits::none())
                .map_err(|e| anyhow::anyhow!("LedgerCloseMeta XDR decode: {e}"))?;

            for change in ledger_changes(&meta) {
                if self.apply(change) {
                    changes_applied += 1;
                }
            }
        }

        if changes_applied > 0 {
            tracing::info!(
                ledger = seq,
                ibc_writes = changes_applied,
                root = %hex::encode(self.smt.root()),
                "[gateway] SMT updated — committed IBC state change(s)"
            );
        } else {
            tracing::debug!(sequence = seq, "ledger processed into smt (no ibc changes)");
        }

        Ok(())
    }

    fn apply(&mut self, change: LedgerEntryChange) -> bool {
        match change {
            LedgerEntryChange::Created(e) => {
                self.apply_contract_data_write(e.data, /* is_update */ false)
            }
            LedgerEntryChange::Updated(e) => {
                self.apply_contract_data_write(e.data, /* is_update */ true)
            }
            LedgerEntryChange::Removed(LedgerKey::ContractData(key)) => {
                if !self.matches(&key.contract) {
                    return false;
                }
                let Some(path) = scval_to_v2_path(&key.key) else {
                    return false;
                };
                self.smt.remove(&path);
                true
            }
            _ => false,
        }
    }

    fn apply_contract_data_write(&mut self, data: LedgerEntryData, is_update: bool) -> bool {
        let LedgerEntryData::ContractData(d) = data else {
            return false;
        };
        if !self.matches(&d.contract) {
            return false;
        }
        let Some(path) = scval_to_v2_path(&d.key) else {
            return false;
        };
        let Some(value) = scval_as_bytes(&d.val) else {
            return false;
        };
        if is_update {
            self.smt.update(&path, &value);
        } else {
            self.smt.insert(&path, &value);
        }
        true
    }

    fn matches(&self, addr: &ScAddress) -> bool {
        let ScAddress::Contract(hash) = addr else {
            return false;
        };
        match &self.ibc_contract_id {
            None => true,
            Some(id) => hash.0 == soroban_client::xdr::Hash(*id),
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
        TransactionMeta::V0(operations) => {
            for op in operations.iter() {
                out.extend(op.changes.iter().cloned());
            }
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
        TransactionMeta::V4(v) => {
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
