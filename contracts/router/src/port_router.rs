use soroban_sdk::{panic_with_error, Address, Env, String};

use crate::errors::Error;
use crate::identifiers::validate_port_id;
use crate::types::DataKey;

pub(crate) fn register_port(env: &Env, port_id: String, app_address: Address) {
    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .unwrap_or_else(|| panic_with_error!(env, Error::AdminNotSet));
    admin.require_auth();
    validate_port_id(env, &port_id);
    let key = DataKey::Port(port_id);
    if env.storage().persistent().has(&key) {
        panic_with_error!(env, Error::PortAlreadyRegistered);
    }
    env.storage().persistent().set(&key, &app_address);
}

pub(crate) fn port_app(env: &Env, port_id: String) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Port(port_id))
}
