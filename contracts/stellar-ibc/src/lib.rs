#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, IntoVal, String, Symbol,
    Val, Vec,
};

const PACKET_COMMITMENT_DISCRIMINATOR: u8 = 0x01;
const PACKET_RECEIPT_DISCRIMINATOR: u8 = 0x02;
const ACK_COMMITMENT_DISCRIMINATOR: u8 = 0x03;

const RECEIPT_SENTINEL: u8 = 0x01;

const PROVABLE_TTL_THRESHOLD: u32 = 17_280;
const PROVABLE_TTL_EXTEND_TO: u32 = 86_400;

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
}

#[contract]
pub struct IbcRouter;

#[contractimpl]
impl IbcRouter {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn register_client_type(env: Env, client_type: String, lc_address: Address) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("admin not set");
        admin.require_auth();

        env.storage()
            .persistent()
            .set(&DataKey::ClientTypeAddr(client_type), &lc_address);
    }

    pub fn lc_address(env: Env, client_type: String) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::ClientTypeAddr(client_type))
    }

    pub fn create_client(
        env: Env,
        client_type: String,
        client_state: Bytes,
        consensus_state: Bytes,
        height: u64,
    ) -> String {
        let lc_addr: Address = env
            .storage()
            .persistent()
            .get(&DataKey::ClientTypeAddr(client_type.clone()))
            .expect("unknown client_type");

        let client_id = mint_client_id(&env, &client_type);

        let args: Vec<Val> = soroban_sdk::vec![
            &env,
            client_id.clone().into_val(&env),
            client_state.into_val(&env),
            consensus_state.into_val(&env),
            height.into_val(&env),
        ];
        let _: () = env.invoke_contract(&lc_addr, &Symbol::new(&env, "initialise"), args);

        env.storage()
            .persistent()
            .set(&DataKey::ClientType(client_id.clone()), &client_type);
        env.storage()
            .persistent()
            .set(&DataKey::ClientLcAddr(client_id.clone()), &lc_addr);
        client_id
    }

    pub fn register_counterparty(
        env: Env,
        client_id: String,
        counterparty_client_id: String,
        counterparty_commitment_prefix: Vec<Bytes>,
    ) {
        if !env
            .storage()
            .persistent()
            .has(&DataKey::ClientType(client_id.clone()))
        {
            panic!("client_id not found");
        }
        if env
            .storage()
            .persistent()
            .has(&DataKey::Counterparty(client_id.clone()))
        {
            panic!("counterparty already registered");
        }
        let cp = Counterparty {
            client_id: counterparty_client_id,
            commitment_prefix: counterparty_commitment_prefix,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Counterparty(client_id), &cp);
    }

    pub fn counterparty(env: Env, client_id: String) -> Option<Counterparty> {
        env.storage()
            .persistent()
            .get(&DataKey::Counterparty(client_id))
    }

    pub fn update_client(env: Env, client_id: String, client_message: Bytes) -> u64 {
        if env
            .storage()
            .persistent()
            .get::<DataKey, bool>(&DataKey::Frozen(client_id.clone()))
            .unwrap_or(false)
        {
            panic!("client frozen");
        }

        let lc_addr: Address = env
            .storage()
            .persistent()
            .get(&DataKey::ClientLcAddr(client_id.clone()))
            .expect("client_id not found");

        let misbehaviour_args: Vec<Val> = soroban_sdk::vec![
            &env,
            client_id.clone().into_val(&env),
            client_message.clone().into_val(&env),
        ];
        let misbehaviour: bool = env.invoke_contract(
            &lc_addr,
            &Symbol::new(&env, "check_for_misbehaviour"),
            misbehaviour_args,
        );

        if misbehaviour {
            let args: Vec<Val> = soroban_sdk::vec![
                &env,
                client_id.clone().into_val(&env),
                client_message.into_val(&env),
            ];
            let _: () = env.invoke_contract(
                &lc_addr,
                &Symbol::new(&env, "update_state_on_misbehaviour"),
                args,
            );
            env.storage()
                .persistent()
                .set(&DataKey::Frozen(client_id), &true);
            return 0;
        }

        let args: Vec<Val> = soroban_sdk::vec![
            &env,
            client_id.into_val(&env),
            client_message.into_val(&env),
        ];
        env.invoke_contract(&lc_addr, &Symbol::new(&env, "update_state"), args)
    }

    pub fn client_lc_address(env: Env, client_id: String) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::ClientLcAddr(client_id))
    }

    pub fn frozen(env: Env, client_id: String) -> bool {
        env.storage()
            .persistent()
            .get(&DataKey::Frozen(client_id))
            .unwrap_or(false)
    }

    pub fn set_packet_commitment(
        env: Env,
        source_client_id: String,
        sequence: u64,
        commitment: BytesN<32>,
    ) {
        let key = packet_commitment_key(&env, &source_client_id, sequence);
        let storage = env.storage().persistent();
        storage.set(&key, &commitment);
        storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
    }

    pub fn packet_commitment(
        env: Env,
        source_client_id: String,
        sequence: u64,
    ) -> Option<BytesN<32>> {
        let key = packet_commitment_key(&env, &source_client_id, sequence);
        env.storage().persistent().get(&key)
    }

    pub fn delete_packet_commitment(env: Env, source_client_id: String, sequence: u64) {
        let key = packet_commitment_key(&env, &source_client_id, sequence);
        env.storage().persistent().remove(&key);
    }

    pub fn set_packet_receipt(env: Env, dest_client_id: String, sequence: u64) {
        let key = packet_receipt_key(&env, &dest_client_id, sequence);
        let sentinel = Bytes::from_slice(&env, &[RECEIPT_SENTINEL]);
        let storage = env.storage().persistent();
        storage.set(&key, &sentinel);
        storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
    }

    pub fn has_packet_receipt(env: Env, dest_client_id: String, sequence: u64) -> bool {
        let key = packet_receipt_key(&env, &dest_client_id, sequence);
        env.storage().persistent().has(&key)
    }

    pub fn set_ack_commitment(
        env: Env,
        dest_client_id: String,
        sequence: u64,
        ack_hash: BytesN<32>,
    ) {
        let key = ack_commitment_key(&env, &dest_client_id, sequence);
        let storage = env.storage().persistent();
        storage.set(&key, &ack_hash);
        storage.extend_ttl(&key, PROVABLE_TTL_THRESHOLD, PROVABLE_TTL_EXTEND_TO);
    }

    pub fn acknowledgement(
        env: Env,
        dest_client_id: String,
        sequence: u64,
    ) -> Option<BytesN<32>> {
        let key = ack_commitment_key(&env, &dest_client_id, sequence);
        env.storage().persistent().get(&key)
    }
}

fn packet_commitment_key(env: &Env, source_client_id: &String, sequence: u64) -> Bytes {
    v2_path_key(env, source_client_id, PACKET_COMMITMENT_DISCRIMINATOR, sequence)
}

fn packet_receipt_key(env: &Env, dest_client_id: &String, sequence: u64) -> Bytes {
    v2_path_key(env, dest_client_id, PACKET_RECEIPT_DISCRIMINATOR, sequence)
}

fn ack_commitment_key(env: &Env, dest_client_id: &String, sequence: u64) -> Bytes {
    v2_path_key(env, dest_client_id, ACK_COMMITMENT_DISCRIMINATOR, sequence)
}

fn v2_path_key(env: &Env, client_id: &String, discriminator: u8, sequence: u64) -> Bytes {
    let id_len = client_id.len() as usize;
    let mut buf = [0u8; 128];
    client_id.copy_into_slice(&mut buf[..id_len]);
    buf[id_len] = discriminator;
    let seq_bytes = sequence.to_be_bytes();
    buf[id_len + 1..id_len + 9].copy_from_slice(&seq_bytes);
    Bytes::from_slice(env, &buf[..id_len + 9])
}

fn mint_client_id(env: &Env, client_type: &String) -> String {
    let key = DataKey::NextClientId(client_type.clone());
    let n: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(n + 1));

    let prefix_len = client_type.len() as usize;
    let mut buf = [0u8; 64];
    client_type.copy_into_slice(&mut buf[..prefix_len]);
    let mut len = prefix_len;
    buf[len] = b'-';
    len += 1;
    len += write_u32(&mut buf[len..], n);

    String::from_bytes(env, &buf[..len])
}

fn write_u32(buf: &mut [u8], mut n: u32) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 10];
    let mut i = 0;
    while n > 0 {
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = tmp[i - 1 - j];
    }
    i
}

mod test;
