use soroban_sdk::{Address, Env, String};

use crate::types::DataKey;

pub(crate) fn register_port(env: &Env, port_id: String, app_address: Address) {
    app_address.require_auth();
    let key = DataKey::Port(port_id);
    if env.storage().persistent().has(&key) {
        panic!("port already registered");
    }
    env.storage().persistent().set(&key, &app_address);
}

pub(crate) fn port_app(env: &Env, port_id: String) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Port(port_id))
}
