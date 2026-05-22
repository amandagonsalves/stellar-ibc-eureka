use soroban_sdk::{contracttype, Bytes, String, Vec};

#[contracttype]
#[derive(Clone)]
pub struct Counterparty {
    pub client_id: String,
    pub commitment_prefix: Vec<Bytes>,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    NextClientId(String),
    ClientTypeAddr(String),
    ClientType(String),
    ClientLcAddr(String),
    Counterparty(String),
    Frozen(String),
    Port(String),
}
