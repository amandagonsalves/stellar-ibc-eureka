use anyhow::{anyhow, Result};
use ibc::clients::tendermint::client_state::ClientState as TmClientState;
use ibc::core::commitment_types::specs::ProofSpecs;
use ibc_proto::google::protobuf::Duration;
use ibc_proto::ibc::core::client::v1::Height as RawHeight;
use ibc_proto::ibc::lightclients::tendermint::v1::{ClientState as RawTmClientState, Fraction};
use stellar_xdr::curr::{
    Limits, ReadXdr, ScBytes, ScMap, ScMapEntry, ScString, ScSymbol, ScVal, StringM, VecM, WriteXdr,
};

use super::client_state::AnyClientState;
use super::consensus_state::AnyConsensusState;

fn sc_field<'a>(map: &'a ScMap, name: &str) -> Result<&'a ScVal> {
    map.0
        .iter()
        .find(|e| matches!(&e.key, ScVal::Symbol(ScSymbol(s)) if s.to_string() == name))
        .map(|e| &e.val)
        .ok_or_else(|| anyhow!("missing client state field: {name}"))
}

fn sc_as_map<'a>(val: &'a ScVal, name: &str) -> Result<&'a ScMap> {
    match val {
        ScVal::Map(Some(m)) => Ok(m),
        _ => Err(anyhow!("field {name} is not a map")),
    }
}

fn sc_u64(map: &ScMap, name: &str) -> Result<u64> {
    match sc_field(map, name)? {
        ScVal::U64(n) => Ok(*n),
        _ => Err(anyhow!("field {name} is not u64")),
    }
}

fn sc_u32(map: &ScMap, name: &str) -> Result<u32> {
    match sc_field(map, name)? {
        ScVal::U32(n) => Ok(*n),
        _ => Err(anyhow!("field {name} is not u32")),
    }
}

fn sc_bool(map: &ScMap, name: &str) -> Result<bool> {
    match sc_field(map, name)? {
        ScVal::Bool(b) => Ok(*b),
        _ => Err(anyhow!("field {name} is not bool")),
    }
}

fn sc_str(map: &ScMap, name: &str) -> Result<String> {
    match sc_field(map, name)? {
        ScVal::String(ScString(s)) => Ok(s.to_string()),
        _ => Err(anyhow!("field {name} is not a string")),
    }
}

fn sc_get_height(map: &ScMap, name: &str) -> Result<RawHeight> {
    let h = sc_as_map(sc_field(map, name)?, name)?;
    Ok(RawHeight {
        revision_number: sc_u64(h, "revision_number")?,
        revision_height: sc_u64(h, "revision_height")?,
    })
}

fn sc_symbol(s: &str) -> Result<ScVal> {
    let m: StringM<32> = s
        .try_into()
        .map_err(|_| anyhow!("invalid struct field symbol: {s}"))?;
    Ok(ScVal::Symbol(ScSymbol(m)))
}

fn sc_string(s: &str) -> Result<ScVal> {
    let m: StringM = s
        .try_into()
        .map_err(|_| anyhow!("invalid string for ScVal"))?;
    Ok(ScVal::String(ScString(m)))
}

fn sc_bytes(b: Vec<u8>) -> Result<ScVal> {
    let bytes: ScBytes = b
        .try_into()
        .map_err(|_| anyhow!("invalid bytes for ScVal"))?;
    Ok(ScVal::Bytes(bytes))
}

fn sc_struct(fields: Vec<(&str, ScVal)>) -> Result<ScVal> {
    let mut entries = Vec::with_capacity(fields.len());
    for (key, val) in fields {
        entries.push(ScMapEntry {
            key: sc_symbol(key)?,
            val,
        });
    }
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    let vm: VecM<ScMapEntry> = entries
        .try_into()
        .map_err(|_| anyhow!("struct map too large"))?;
    Ok(ScVal::Map(Some(ScMap(vm))))
}

fn sc_height(revision_number: u64, revision_height: u64) -> Result<ScVal> {
    sc_struct(vec![
        ("revision_number", ScVal::U64(revision_number)),
        ("revision_height", ScVal::U64(revision_height)),
    ])
}

impl AnyClientState {
    pub fn to_soroban_xdr(&self) -> Result<Vec<u8>> {
        let AnyClientState::Tendermint(cs) = self;
        let cs = cs.inner();

        let trust_level = sc_struct(vec![
            ("numerator", ScVal::U32(cs.trust_level.numerator() as u32)),
            (
                "denominator",
                ScVal::U32(cs.trust_level.denominator() as u32),
            ),
        ])?;

        let (frozen_rn, frozen_rh) = cs
            .frozen_height
            .as_ref()
            .map(|h| (h.revision_number(), h.revision_height()))
            .unwrap_or((0, 0));

        let state = sc_struct(vec![
            ("chain_id", sc_string(cs.chain_id.as_str())?),
            ("trust_level", trust_level),
            (
                "trusting_period_secs",
                ScVal::U64(cs.trusting_period.as_secs()),
            ),
            (
                "unbonding_period_secs",
                ScVal::U64(cs.unbonding_period.as_secs()),
            ),
            (
                "max_clock_drift_secs",
                ScVal::U64(cs.max_clock_drift.as_secs()),
            ),
            (
                "latest_height",
                sc_height(
                    cs.latest_height.revision_number(),
                    cs.latest_height.revision_height(),
                )?,
            ),
            ("is_frozen", ScVal::Bool(cs.frozen_height.is_some())),
            ("frozen_height", sc_height(frozen_rn, frozen_rh)?),
            ("proof_specs", sc_bytes(Vec::new())?),
        ])?;

        state
            .to_xdr(Limits::none())
            .map_err(|e| anyhow!("client_state to_xdr: {e}"))
    }

    pub fn from_soroban_xdr(bytes: &[u8]) -> Result<Self> {
        let val = ScVal::from_xdr(bytes, Limits::none())
            .map_err(|e| anyhow!("client_state from_xdr: {e}"))?;
        let map = sc_as_map(&val, "client_state")?;

        let trust = sc_as_map(sc_field(map, "trust_level")?, "trust_level")?;
        let is_frozen = sc_bool(map, "is_frozen")?;
        let frozen_height = if is_frozen {
            Some(sc_get_height(map, "frozen_height")?)
        } else {
            None
        };

        let raw = RawTmClientState {
            chain_id: sc_str(map, "chain_id")?,
            trust_level: Some(Fraction {
                numerator: sc_u32(trust, "numerator")? as u64,
                denominator: sc_u32(trust, "denominator")? as u64,
            }),
            trusting_period: Some(Duration {
                seconds: sc_u64(map, "trusting_period_secs")? as i64,
                nanos: 0,
            }),
            unbonding_period: Some(Duration {
                seconds: sc_u64(map, "unbonding_period_secs")? as i64,
                nanos: 0,
            }),
            max_clock_drift: Some(Duration {
                seconds: sc_u64(map, "max_clock_drift_secs")? as i64,
                nanos: 0,
            }),
            frozen_height,
            latest_height: Some(sc_get_height(map, "latest_height")?),
            proof_specs: ProofSpecs::cosmos().into(),
            upgrade_path: vec!["upgrade".to_string(), "upgradedIBCState".to_string()],
            allow_update_after_expiry: false,
            allow_update_after_misbehaviour: false,
        };

        let cs = TmClientState::try_from(raw)
            .map_err(|e| anyhow!("tendermint client state from raw: {e}"))?;

        Ok(AnyClientState::Tendermint(cs))
    }
}

impl AnyConsensusState {
    pub fn to_soroban_xdr(&self) -> Result<Vec<u8>> {
        let AnyConsensusState::Tendermint(cons) = self;
        let cons = cons.inner();

        let timestamp_secs = cons.timestamp.unix_timestamp().max(0) as u64;
        let next_validators_hash = cons.next_validators_hash.as_bytes().to_vec();
        let root = cons.root.as_bytes().to_vec();

        let state = sc_struct(vec![
            ("timestamp_secs", ScVal::U64(timestamp_secs)),
            ("next_validators_hash", sc_bytes(next_validators_hash)?),
            ("root", sc_bytes(root)?),
        ])?;

        state
            .to_xdr(Limits::none())
            .map_err(|e| anyhow!("consensus_state to_xdr: {e}"))
    }
}
