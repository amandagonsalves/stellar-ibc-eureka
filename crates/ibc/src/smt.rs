use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const EMPTY: [u8; 32] = [0u8; 32];

pub struct Smt {
    leaves: BTreeMap<[u8; 32], [u8; 32]>,
}

impl Default for Smt {
    fn default() -> Self {
        Self::new()
    }
}

impl Smt {
    pub fn new() -> Self {
        Self {
            leaves: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, key: &[u8], value: &[u8]) {
        let k: [u8; 32] = Sha256::digest(key).into();
        let v: [u8; 32] = Sha256::digest(value).into();
        self.leaves.insert(k, v);
    }

    pub fn remove(&mut self, key: &[u8]) {
        let k: [u8; 32] = Sha256::digest(key).into();
        self.leaves.remove(&k);
    }

    pub fn root(&self) -> [u8; 32] {
        let leaves: Vec<([u8; 32], [u8; 32])> =
            self.leaves.iter().map(|(&k, &v)| (k, v)).collect();
        subtree_root(&leaves, 0)
    }
}

fn bit_at(key: &[u8; 32], depth: u8) -> u8 {
    (key[(depth / 8) as usize] >> (7 - depth % 8)) & 1
}

fn leaf_hash(key: [u8; 32], val: [u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0x00]);
    h.update(key);
    h.update(val);
    h.finalize().into()
}

fn inner_hash(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([0x01]);
    h.update(left);
    h.update(right);
    h.finalize().into()
}

fn subtree_root(leaves: &[([u8; 32], [u8; 32])], depth: u8) -> [u8; 32] {
    if leaves.is_empty() {
        return EMPTY;
    }
    if leaves.len() == 1 {
        return leaf_hash(leaves[0].0, leaves[0].1);
    }
    if depth == u8::MAX {
        return leaf_hash(leaves[0].0, leaves[0].1);
    }
    let mid = leaves.partition_point(|&(k, _)| bit_at(&k, depth) == 0);
    let left = subtree_root(&leaves[..mid], depth + 1);
    let right = subtree_root(&leaves[mid..], depth + 1);
    inner_hash(left, right)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_root_is_zeros() {
        assert_eq!(Smt::new().root(), EMPTY);
    }

    #[test]
    fn single_entry_root_is_leaf_hash() {
        let mut smt = Smt::new();
        smt.insert(b"key", b"val");
        let k: [u8; 32] = Sha256::digest(b"key").into();
        let v: [u8; 32] = Sha256::digest(b"val").into();
        assert_eq!(smt.root(), leaf_hash(k, v));
    }

    #[test]
    fn insert_remove_restores_root() {
        let mut smt = Smt::new();
        smt.insert(b"k1", b"v1");
        let root_before = smt.root();
        smt.insert(b"k2", b"v2");
        smt.remove(b"k2");
        assert_eq!(smt.root(), root_before);
    }

    #[test]
    fn two_entries_root_differs_from_single() {
        let mut smt = Smt::new();
        smt.insert(b"k1", b"v1");
        let r1 = smt.root();
        smt.insert(b"k2", b"v2");
        let r2 = smt.root();
        assert_ne!(r1, r2);
    }
}
