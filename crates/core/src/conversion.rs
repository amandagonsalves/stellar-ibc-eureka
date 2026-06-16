use anyhow::{anyhow, Result};
use soroban_client::xdr::{
    ContractDataDurability, ContractId, Hash, LedgerEntryData, LedgerKey, LedgerKeyContractData,
    Limits, ReadXdr, ScAddress, ScBytes, ScMap, ScMapEntry, ScString, ScSymbol, ScVal, ScVec,
    StringM, VecM, WriteXdr,
};

pub fn scval_string(s: &str) -> Result<ScVal> {
    let m: StringM = s
        .try_into()
        .map_err(|_| anyhow!("invalid string for ScVal: {s}"))?;

    Ok(ScVal::String(ScString(m)))
}

pub fn scval_symbol(s: &str) -> Result<ScVal> {
    let m: StringM<32> = s
        .try_into()
        .map_err(|_| anyhow!("invalid symbol for ScVal: {s}"))?;

    Ok(ScVal::Symbol(ScSymbol(m)))
}

pub fn scval_bytes(b: &[u8]) -> Result<ScVal> {
    let bytes: ScBytes = b
        .to_vec()
        .try_into()
        .map_err(|_| anyhow!("invalid bytes for ScVal"))?;

    Ok(ScVal::Bytes(bytes))
}

pub fn scval_u64(v: u64) -> ScVal {
    ScVal::U64(v)
}

pub fn scval_vec(items: Vec<ScVal>) -> Result<ScVal> {
    let vm: VecM<ScVal> = items
        .try_into()
        .map_err(|_| anyhow!("ScVal vec too large"))?;

    Ok(ScVal::Vec(Some(ScVec(vm))))
}

pub fn scval_vec_of_bytes(items: &[Vec<u8>]) -> Result<ScVal> {
    let inner = items
        .iter()
        .map(|b| scval_bytes(b))
        .collect::<Result<Vec<_>>>()?;

    scval_vec(inner)
}

pub fn scval_struct(fields: Vec<(&str, ScVal)>) -> Result<ScVal> {
    let mut entries = Vec::with_capacity(fields.len());
    for (key, val) in fields {
        entries.push(ScMapEntry {
            key: scval_symbol(key)?,
            val,
        });
    }
    entries.sort_by(|a, b| a.key.cmp(&b.key));

    let vm: VecM<ScMapEntry> = entries
        .try_into()
        .map_err(|_| anyhow!("struct map too large"))?;

    Ok(ScVal::Map(Some(ScMap(vm))))
}

pub fn scval_height(revision_number: u64, revision_height: u64) -> Result<ScVal> {
    scval_struct(vec![
        ("revision_number", ScVal::U64(revision_number)),
        ("revision_height", ScVal::U64(revision_height)),
    ])
}

pub fn scval_from_xdr(bytes: &[u8]) -> Result<ScVal> {
    ScVal::from_xdr(bytes, Limits::none()).map_err(|e| anyhow!("ScVal from_xdr: {e}"))
}

pub fn scval_to_xdr(val: &ScVal) -> Result<Vec<u8>> {
    val.to_xdr(Limits::none()).map_err(|e| anyhow!("ScVal to_xdr: {e}"))
}

pub fn persistent_contract_data_key(contract: [u8; 32], key_val: ScVal) -> Result<Vec<u8>> {
    let key = LedgerKey::ContractData(LedgerKeyContractData {
        contract: ScAddress::Contract(ContractId(Hash(contract))),
        key: key_val,
        durability: ContractDataDurability::Persistent,
    });

    key.to_xdr(Limits::none())
        .map_err(|e| anyhow!("ledger key to_xdr: {e}"))
}

pub fn ledger_entry_contract_val(entry_xdr: &[u8]) -> Option<ScVal> {
    match LedgerEntryData::from_xdr(entry_xdr, Limits::none()).ok()? {
        LedgerEntryData::ContractData(d) => Some(d.val),
        _ => None,
    }
}

pub fn scval_field<'a>(map: &'a ScMap, name: &str) -> Option<&'a ScVal> {
    map.0
        .iter()
        .find(|e| matches!(&e.key, ScVal::Symbol(ScSymbol(s)) if s.to_string() == name))
        .map(|e| &e.val)
}

pub fn scval_as_map(val: &ScVal) -> Option<&ScMap> {
    match val {
        ScVal::Map(Some(m)) => Some(m),
        _ => None,
    }
}

pub fn scval_as_u64(val: &ScVal) -> Option<u64> {
    match val {
        ScVal::U64(n) => Some(*n),
        _ => None,
    }
}

pub fn scval_as_u32(val: &ScVal) -> Option<u32> {
    match val {
        ScVal::U32(n) => Some(*n),
        _ => None,
    }
}

pub fn scval_as_string(val: &ScVal) -> Option<String> {
    match val {
        ScVal::String(ScString(s)) => Some(s.to_string()),
        _ => None,
    }
}

pub fn scval_as_symbol(val: &ScVal) -> Option<String> {
    match val {
        ScVal::Symbol(ScSymbol(s)) => Some(s.to_string()),
        _ => None,
    }
}

pub fn scval_as_bytes(val: &ScVal) -> Option<Vec<u8>> {
    match val {
        ScVal::Bytes(b) => Some(b.to_vec()),
        _ => None,
    }
}

pub fn scval_as_i128(val: &ScVal) -> Option<i128> {
    match val {
        ScVal::I128(parts) => Some(((parts.hi as i128) << 64) | (parts.lo as i128)),
        _ => None,
    }
}

pub fn scval_map_field<'a>(map: &'a ScMap, name: &str) -> Result<&'a ScVal> {
    scval_field(map, name).ok_or_else(|| anyhow!("missing field: {name}"))
}

pub fn scval_map_as_map<'a>(map: &'a ScMap, name: &str) -> Result<&'a ScMap> {
    scval_as_map(scval_map_field(map, name)?).ok_or_else(|| anyhow!("field {name} is not a map"))
}

pub fn scval_map_u64(map: &ScMap, name: &str) -> Result<u64> {
    scval_as_u64(scval_map_field(map, name)?).ok_or_else(|| anyhow!("field {name} is not u64"))
}

pub fn scval_map_u32(map: &ScMap, name: &str) -> Result<u32> {
    scval_as_u32(scval_map_field(map, name)?).ok_or_else(|| anyhow!("field {name} is not u32"))
}

pub fn scval_map_string(map: &ScMap, name: &str) -> Result<String> {
    scval_as_string(scval_map_field(map, name)?)
        .ok_or_else(|| anyhow!("field {name} is not a string"))
}

pub fn scval_map_bytes(map: &ScMap, name: &str) -> Result<Vec<u8>> {
    scval_as_bytes(scval_map_field(map, name)?).ok_or_else(|| anyhow!("field {name} is not bytes"))
}
