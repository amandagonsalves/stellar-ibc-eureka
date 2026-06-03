use cosmwasm_std::Binary;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Height {
    pub revision_number: u64,
    pub revision_height: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstantiateMsg {
    pub client_state: Binary,
    pub consensus_state: Binary,
    pub checksum: Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SudoMsg {
    UpdateState(UpdateStateMsg),
    UpdateStateOnMisbehaviour(UpdateStateOnMisbehaviourMsg),
    CheckForMisbehaviour(CheckForMisbehaviourMsg),
    VerifyMembership(VerifyMembershipMsg),
    VerifyNonMembership(VerifyNonMembershipMsg),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStateMsg {
    pub client_message: Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStateOnMisbehaviourMsg {
    pub client_message: Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckForMisbehaviourMsg {
    pub client_message: Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyMembershipMsg {
    pub height: Height,
    pub delay_time_period: u64,
    pub delay_block_period: u64,
    pub proof: Binary,
    pub path: Vec<Binary>,
    pub value: Binary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerifyNonMembershipMsg {
    pub height: Height,
    pub delay_time_period: u64,
    pub delay_block_period: u64,
    pub proof: Binary,
    pub path: Vec<Binary>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    ClientState {},
    ConsensusState { height: Height },
    LatestHeight {},
    Status {},
    TimestampAtHeight { height: Height },
    VerifyClientMessage { client_message: Binary },
    CheckForMisbehaviour { client_message: Binary },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStateResult {
    pub heights: Vec<Height>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckForMisbehaviourResult {
    pub found_misbehaviour: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusResult {
    pub status: ClientStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClientStatus {
    Active,
    Frozen,
    Expired,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LatestHeightResult {
    pub height: Height,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimestampAtHeightResult {
    pub timestamp: u64,
}
