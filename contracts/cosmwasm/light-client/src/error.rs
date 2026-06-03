use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("client state already initialised")]
    AlreadyInitialised,

    #[error("client state not initialised")]
    NotInitialised,

    #[error("client is frozen at height {height}")]
    Frozen { height: u64 },

    #[error("invalid wire bytes: {0}")]
    InvalidWire(String),

    #[error("consensus state missing at height {height}")]
    ConsensusStateMissing { height: u64 },

    #[error("header chain_id {got} does not match client chain_id {expected}")]
    ChainIdMismatch { expected: String, got: String },

    #[error("header height {target} must be greater than trusted height {trusted}")]
    NonAdvancingHeight { trusted: u64, target: u64 },

    #[error("conflicting consensus state already stored at height {height}")]
    ConsensusStateConflict { height: u64 },

    #[error(
        "scp quorum not met (envelopes={envelopes}, matched_trusted={matched}, verified={verified}, signer={signer}, trusted=[{trusted}])"
    )]
    QuorumNotMet {
        envelopes: usize,
        matched: usize,
        verified: usize,
        signer: String,
        trusted: String,
    },

    #[error("scp network_id is not configured on the client state")]
    NetworkIdMissing,

    #[error("scp signature verification error: {0}")]
    ScpSignatureError(String),

    #[error("merkle proof verification failed")]
    MerkleVerificationFailed,

    #[error("unknown sudo message variant")]
    UnknownSudo,
}
