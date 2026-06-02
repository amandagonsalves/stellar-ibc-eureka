use std::collections::HashMap;

use sha2::{Digest, Sha256};
use soroban_client::xdr::{
    LedgerCloseMeta, LedgerEntryChange, LedgerEntryData, LedgerKey, Limits, ReadXdr, ScAddress,
    ScVal, TransactionMeta,
};
use stellar_ibc_core::api_client::ApiClient;
use stellar_ibc_core::proof::{
    serialize_membership_proof_with_index, serialize_non_membership_proof_with_index,
};
use stellar_ibc_core::smt::Smt;

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

pub struct StateTracker {
    api: ApiClient,
    roots: HashMap<u32, [u8; 32]>,
    ibc_contract_id: Option<[u8; 32]>,
    smt: Smt,
}

impl StateTracker {
    pub fn new(api: ApiClient, ibc_contract_id: Option<[u8; 32]>) -> Self {
        Self {
            api,
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
            None => {
                let proof = self
                    .smt
                    .generate_non_membership_proof(key)
                    .ok_or_else(|| anyhow::anyhow!("non-membership proof unavailable for key"))?;
                let bytes = serialize_non_membership_proof_with_index(&proof, key, index);
                Ok(PathLookup::Absent { proof_bytes: bytes })
            }
        }
    }

    async fn process(&mut self, seq: u32) -> anyhow::Result<[u8; 32]> {
        tracing::debug!(sequence = seq, "fetching ledger via api");
        let ledger = self.api.get_ledger(seq).await?;

        let mut changes_applied = 0usize;
        if let Some(meta_xdr) = ledger.metadata_xdr {
            let meta = LedgerCloseMeta::from_xdr(&meta_xdr, Limits::none())
                .map_err(|e| anyhow::anyhow!("LedgerCloseMeta XDR decode: {e}"))?;

            for change in ledger_changes(&meta) {
                self.apply(change);
                changes_applied += 1;
            }
        }

        let root = self.smt.root();
        self.roots.insert(seq, root);
        if changes_applied > 0 {
            tracing::info!(
                sequence = seq,
                changes_applied,
                root = %hex::encode(root),
                "ledger applied ibc state changes into smt"
            );
        } else {
            tracing::debug!(
                sequence = seq,
                changes_applied,
                root = %hex::encode(root),
                "ledger processed into smt (no ibc changes)"
            );
        }
        Ok(root)
    }

    fn apply(&mut self, change: LedgerEntryChange) {
        match change {
            LedgerEntryChange::Created(e) => {
                self.apply_contract_data_write(e.data, /* is_update */ false);
            }
            LedgerEntryChange::Updated(e) => {
                self.apply_contract_data_write(e.data, /* is_update */ true);
            }
            LedgerEntryChange::Removed(LedgerKey::ContractData(key)) => {
                if !self.matches(&key.contract) {
                    return;
                }
                let Some(path) = scval_to_v2_path(&key.key) else {
                    return;
                };
                self.smt.remove(&path);
            }
            _ => {}
        }
    }

    fn apply_contract_data_write(&mut self, data: LedgerEntryData, is_update: bool) {
        let LedgerEntryData::ContractData(d) = data else {
            return;
        };
        if !self.matches(&d.contract) {
            return;
        }
        let Some(path) = scval_to_v2_path(&d.key) else {
            return;
        };
        let Some(value) = scval_to_provable_value(&d.val) else {
            return;
        };
        if is_update {
            self.smt.update(&path, &value);
        } else {
            self.smt.insert(&path, &value);
        }
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
        #[allow(unreachable_patterns)]
        _ => {}
    }
}

fn scval_to_v2_path(key: &ScVal) -> Option<Vec<u8>> {
    let ScVal::Bytes(bytes) = key else {
        return None;
    };
    if !is_v2_provable_path(bytes.as_slice()) {
        return None;
    }
    Some(bytes.as_slice().to_vec())
}

fn scval_to_provable_value(value: &ScVal) -> Option<Vec<u8>> {
    match value {
        ScVal::Bytes(b) => Some(b.as_slice().to_vec()),
        _ => None,
    }
}

fn is_v2_provable_path(key: &[u8]) -> bool {
    if key.len() < 10 {
        return false;
    }
    let discriminator = key[key.len() - 9];
    matches!(discriminator, 0x01..=0x03)
}
