#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, vec,
    xdr::{FromXdr, ToXdr},
    Address, Bytes, Env, IntoVal, String, Symbol, Vec,
};

#[contracttype]
#[derive(Clone)]
pub struct Payload {
    pub source_port: String,
    pub dest_port: String,
    pub version: String,
    pub encoding: String,
    pub value: Bytes,
}

#[contracttype]
#[derive(Clone)]
pub struct OnRecvPacketCallback {
    pub source_client: String,
    pub dest_client: String,
    pub sequence: u64,
    pub payload: Payload,
}

#[contracttype]
#[derive(Clone)]
pub struct OnAcknowledgementPacketCallback {
    pub source_client: String,
    pub dest_client: String,
    pub sequence: u64,
    pub payload: Payload,
    pub acknowledgement: Bytes,
}

#[contracttype]
#[derive(Clone)]
pub struct OnTimeoutPacketCallback {
    pub source_client: String,
    pub dest_client: String,
    pub sequence: u64,
    pub payload: Payload,
}

const PORT: &str = "transfer";
const VERSION: &str = "ics20-1";
const ENCODING: &str = "application/json";

const SUCCESS_ACK_BYTE: u8 = 0x01;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    RouterNotSet = 1,
    InvalidPacketData = 2,
    InsufficientBalance = 3,
    AmountMustBePositive = 4,
    CallerIsNotRouter = 5,
    UnknownSourcePort = 6,
    UnknownDestPort = 7,
    AdminNotSet = 8,
    DailyLimitExceeded = 9,
}

const SECS_PER_DAY: u64 = 86_400;

#[contracttype]
#[derive(Clone)]
pub struct Token {
    pub denom: String,
    pub amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct FungibleTokenPacketData {
    pub token: Token,
    pub sender: String,
    pub receiver: String,
    pub memo: String,
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Router,
    Admin,
    Balance(Address, String),
    DailyCap(String),
    Usage(String, u64),
}

#[contract]
pub struct IbcTransfer;

#[contractimpl]
impl IbcTransfer {
    pub fn __constructor(env: Env, router: Address, admin: Address) {
        env.storage().instance().set(&DataKey::Router, &router);
        env.storage().instance().set(&DataKey::Admin, &admin);
    }

    pub fn set_rate_limit(env: Env, denom: String, daily_cap: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_with_error!(&env, Error::AdminNotSet));
        admin.require_auth();
        if daily_cap < 0 {
            panic_with_error!(&env, Error::AmountMustBePositive);
        }
        env.storage()
            .persistent()
            .set(&DataKey::DailyCap(denom), &daily_cap);
    }

    pub fn daily_cap(env: Env, denom: String) -> Option<i128> {
        env.storage().persistent().get(&DataKey::DailyCap(denom))
    }

    pub fn daily_usage(env: Env, denom: String) -> i128 {
        let day = current_day(&env);
        env.storage()
            .persistent()
            .get(&DataKey::Usage(denom, day))
            .unwrap_or(0)
    }

    pub fn mint(env: Env, to: Address, denom: String, amount: i128) {
        if amount <= 0 {
            panic_with_error!(&env, Error::AmountMustBePositive);
        }
        let key = DataKey::Balance(to, denom);
        let cur: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(cur + amount));
    }

    pub fn balance_of(env: Env, who: Address, denom: String) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Balance(who, denom))
            .unwrap_or(0)
    }

    pub fn initiate_transfer(
        env: Env,
        sender: Address,
        source_client_id: String,
        denom: String,
        amount: i128,
        receiver: String,
        timeout_timestamp: u64,
        memo: String,
    ) -> u64 {
        sender.require_auth();
        if amount <= 0 {
            panic_with_error!(&env, Error::AmountMustBePositive);
        }

        enforce_rate_limit(&env, &denom, amount);

        let escrow = env.current_contract_address();
        debit(&env, &sender, &denom, amount);
        credit(&env, &escrow, &denom, amount);

        let packet = FungibleTokenPacketData {
            token: Token {
                denom: denom.clone(),
                amount,
            },
            sender: address_to_string(&env, &sender),
            receiver,
            memo,
        };
        let value = encode_ics20_json(&env, &packet);

        let port = String::from_str(&env, PORT);
        let payload = Payload {
            source_port: port.clone(),
            dest_port: port,
            version: String::from_str(&env, VERSION),
            encoding: String::from_str(&env, ENCODING),
            value,
        };

        let router_addr: Address = env
            .storage()
            .instance()
            .get(&DataKey::Router)
            .unwrap_or_else(|| panic_with_error!(&env, Error::RouterNotSet));
        let payloads: Vec<Payload> = vec![&env, payload];
        env.invoke_contract::<u64>(
            &router_addr,
            &Symbol::new(&env, "send_packet"),
            vec![
                &env,
                source_client_id.into_val(&env),
                timeout_timestamp.into_val(&env),
                payloads.into_val(&env),
            ],
        )
    }

    pub fn on_recv_packet(env: Env, callback: OnRecvPacketCallback) -> Bytes {
        require_router(&env);
        let port = String::from_str(&env, PORT);
        if callback.payload.dest_port != port {
            panic_with_error!(&env, Error::UnknownDestPort);
        }

        let packet = decode_packet(&env, &callback.payload.value);

        let receiver_addr = string_to_address(&env, &packet.receiver);
        credit(
            &env,
            &receiver_addr,
            &packet.token.denom,
            packet.token.amount,
        );

        Bytes::from_slice(&env, &[SUCCESS_ACK_BYTE])
    }

    pub fn on_acknowledgement_packet(env: Env, callback: OnAcknowledgementPacketCallback) {
        require_router(&env);
        let port = String::from_str(&env, PORT);
        if callback.payload.source_port != port {
            panic_with_error!(&env, Error::UnknownSourcePort);
        }

        if is_success_ack(&callback.acknowledgement) {
            return;
        }

        let packet = decode_packet(&env, &callback.payload.value);
        refund(&env, &packet);
    }

    pub fn on_timeout_packet(env: Env, callback: OnTimeoutPacketCallback) {
        require_router(&env);
        let port = String::from_str(&env, PORT);
        if callback.payload.source_port != port {
            panic_with_error!(&env, Error::UnknownSourcePort);
        }

        let packet = decode_packet(&env, &callback.payload.value);
        refund(&env, &packet);
    }
}

fn debit(env: &Env, who: &Address, denom: &String, amount: i128) {
    let key = DataKey::Balance(who.clone(), denom.clone());
    let cur: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    if cur < amount {
        panic_with_error!(env, Error::InsufficientBalance);
    }
    env.storage().persistent().set(&key, &(cur - amount));
}

fn credit(env: &Env, who: &Address, denom: &String, amount: i128) {
    let key = DataKey::Balance(who.clone(), denom.clone());
    let cur: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    env.storage().persistent().set(&key, &(cur + amount));
}

fn refund(env: &Env, packet: &FungibleTokenPacketData) {
    let escrow = env.current_contract_address();
    let sender_addr = string_to_address(env, &packet.sender);
    debit(env, &escrow, &packet.token.denom, packet.token.amount);
    credit(env, &sender_addr, &packet.token.denom, packet.token.amount);
}

fn decode_packet(env: &Env, value: &Bytes) -> FungibleTokenPacketData {
    FungibleTokenPacketData::from_xdr(env, value)
        .unwrap_or_else(|_| panic_with_error!(env, Error::InvalidPacketData))
}

pub fn encode_ics20_json(env: &Env, packet: &FungibleTokenPacketData) -> Bytes {
    let mut buf = Bytes::from_slice(env, b"{\"denom\":\"");
    append_string(env, &mut buf, &packet.token.denom);
    buf.append(&Bytes::from_slice(env, b"\",\"amount\":\""));
    append_decimal(env, &mut buf, packet.token.amount);
    buf.append(&Bytes::from_slice(env, b"\",\"sender\":\""));
    append_string(env, &mut buf, &packet.sender);
    buf.append(&Bytes::from_slice(env, b"\",\"receiver\":\""));
    append_string(env, &mut buf, &packet.receiver);
    buf.append(&Bytes::from_slice(env, b"\",\"memo\":\""));
    append_string(env, &mut buf, &packet.memo);
    buf.append(&Bytes::from_slice(env, b"\"}"));
    buf
}

fn append_string(env: &Env, buf: &mut Bytes, s: &String) {
    let len = s.len() as usize;
    let mut tmp = [0u8; 512];
    s.copy_into_slice(&mut tmp[..len]);
    buf.append(&Bytes::from_slice(env, &tmp[..len]));
}

fn append_decimal(env: &Env, buf: &mut Bytes, mut n: i128) {
    if n <= 0 {
        buf.append(&Bytes::from_slice(env, b"0"));

        return;
    }

    let mut digits = [0u8; 40];
    let mut i = 40usize;
    while n > 0 {
        i -= 1;
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }

    buf.append(&Bytes::from_slice(env, &digits[i..]));
}

fn require_router(env: &Env) {
    let router_addr: Address = env
        .storage()
        .instance()
        .get(&DataKey::Router)
        .unwrap_or_else(|| panic_with_error!(env, Error::RouterNotSet));
    router_addr.require_auth();
}

fn is_success_ack(ack: &Bytes) -> bool {
    if ack.len() == 1 && ack.get(0).unwrap_or(0) == SUCCESS_ACK_BYTE {
        return true;
    }

    let prefix: &[u8] = b"{\"result\"";
    if ack.len() < prefix.len() as u32 {
        return false;
    }
    for (i, b) in prefix.iter().enumerate() {
        if ack.get(i as u32).unwrap_or(0) != *b {
            return false;
        }
    }

    true
}

pub fn address_to_string(env: &Env, addr: &Address) -> String {
    let xdr = addr.clone().to_xdr(env);

    let len = xdr.len() as usize;
    let mut buf = [0u8; 256];
    xdr.copy_into_slice(&mut buf[..len]);
    let mut hex = [0u8; 512];
    for i in 0..len {
        hex[i * 2] = nibble_to_hex(buf[i] >> 4);
        hex[i * 2 + 1] = nibble_to_hex(buf[i] & 0x0f);
    }
    String::from_bytes(env, &hex[..len * 2])
}

fn string_to_address(env: &Env, s: &String) -> Address {
    let len = s.len() as usize;
    if len % 2 != 0 || len > 512 {
        panic_with_error!(env, Error::InvalidPacketData);
    }
    let mut hex = [0u8; 512];
    s.copy_into_slice(&mut hex[..len]);
    let bytes_len = len / 2;
    let mut buf = [0u8; 256];
    for i in 0..bytes_len {
        let hi = hex_to_nibble(env, hex[i * 2]);
        let lo = hex_to_nibble(env, hex[i * 2 + 1]);
        buf[i] = (hi << 4) | lo;
    }
    let bytes = Bytes::from_slice(env, &buf[..bytes_len]);
    Address::from_xdr(env, &bytes)
        .unwrap_or_else(|_| panic_with_error!(env, Error::InvalidPacketData))
}

fn nibble_to_hex(n: u8) -> u8 {
    if n < 10 {
        b'0' + n
    } else {
        b'a' + (n - 10)
    }
}

fn hex_to_nibble(env: &Env, c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic_with_error!(env, Error::InvalidPacketData),
    }
}

#[allow(dead_code)]
fn _symbol_kept_in_scope(env: &Env) -> Symbol {
    Symbol::new(env, "noop")
}

fn current_day(env: &Env) -> u64 {
    env.ledger().timestamp() / SECS_PER_DAY
}

fn enforce_rate_limit(env: &Env, denom: &String, amount: i128) {
    let cap: Option<i128> = env
        .storage()
        .persistent()
        .get(&DataKey::DailyCap(denom.clone()));
    let cap = match cap {
        Some(c) => c,
        None => return,
    };

    let day = current_day(env);
    let usage_key = DataKey::Usage(denom.clone(), day);
    let used: i128 = env.storage().persistent().get(&usage_key).unwrap_or(0);
    if used + amount > cap {
        panic_with_error!(env, Error::DailyLimitExceeded);
    }
    env.storage().persistent().set(&usage_key, &(used + amount));
}

mod test;
