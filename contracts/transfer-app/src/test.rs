#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{vec, Address, Bytes, Env, String};
use stellar_ibc_router::{IbcRouter, IbcRouterClient};
use stellar_mock_light_client::MockLightClient;

struct Fixture {
    env: Env,
    router: IbcRouterClient<'static>,
    transfer: IbcTransferAppClient<'static>,
    transfer_addr: Address,
    source_client_id: String,
    counterparty_client_id: String,
    transfer_admin: Address,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let router_admin = Address::generate(&env);
    let router_addr = env.register(IbcRouter, (router_admin,));
    let router = IbcRouterClient::new(&env, &router_addr);
    let lc_id = env.register(MockLightClient, ());
    router.register_client_type(&String::from_str(&env, "mock"), &lc_id);

    let source_client_id = router.create_client(
        &String::from_str(&env, "mock"),
        &Bytes::from_slice(&env, b"cs"),
        &Bytes::from_slice(&env, b"cons"),
        &1,
    );
    let counterparty_client_id = String::from_str(&env, "07-tendermint-0");
    router.register_counterparty(
        &source_client_id,
        &counterparty_client_id,
        &vec![&env, Bytes::from_slice(&env, b"ibc")],
    );

    let transfer_admin = Address::generate(&env);
    let transfer_addr = env.register(IbcTransferApp, (router_addr, transfer_admin.clone()));
    let transfer = IbcTransferAppClient::new(&env, &transfer_addr);

    router.register_port(&String::from_str(&env, "transfer"), &transfer_addr);

    Fixture {
        env,
        router,
        transfer,
        transfer_addr,
        source_client_id,
        counterparty_client_id,
        transfer_admin,
    }
}

fn xlm(env: &Env) -> String {
    String::from_str(env, "XLM")
}

#[test]
fn initiate_transfer_escrows_and_emits_packet() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &1_000);

    let seq = f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &250,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(seq, 1);
    assert_eq!(f.transfer.balance_of(&sender, &xlm(&f.env)), 750);
    assert_eq!(f.transfer.balance_of(&f.transfer_addr, &xlm(&f.env)), 250);

    assert!(f
        .router
        .packet_commitment(&f.source_client_id, &seq)
        .is_some());
}

#[test]
fn initiate_transfer_rejects_insufficient_balance() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &100);

    let result = f.transfer.try_initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &500,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(result, Err(Ok(Error::InsufficientBalance.into())));
}

#[test]
fn initiate_transfer_rejects_zero_amount() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &100);

    let result = f.transfer.try_initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &0,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(result, Err(Ok(Error::AmountMustBePositive.into())));
}


fn build_inbound_packet(
    env: &Env,
    sender_blob: &str,
    receiver: &Address,
    denom: &String,
    amount: i128,
) -> Bytes {
    let pkt = FungibleTokenPacketData {
        token: Token {
            denom: denom.clone(),
            amount,
        },
        sender: String::from_str(env, sender_blob),
        receiver: address_to_string(env, receiver),
        memo: String::from_str(env, ""),
    };
    pkt.to_xdr(env)
}

#[test]
fn recv_packet_credits_receiver_and_returns_success_ack() {
    let f = setup();
    let receiver = Address::generate(&f.env);

    let packet = stellar_ibc_router::Packet {
        sequence: 1,
        source_client: f.counterparty_client_id.clone(),
        dest_client: f.source_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![
            &f.env,
            stellar_ibc_router::Payload {
                source_port: String::from_str(&f.env, "transfer"),
                dest_port: String::from_str(&f.env, "transfer"),
                version: String::from_str(&f.env, "ics20-2"),
                encoding: String::from_str(&f.env, "xdr"),
                value: build_inbound_packet(&f.env, "cosmos1abc", &receiver, &xlm(&f.env), 500i128),
            },
        ],
    };

    f.env.ledger().set_timestamp(1_000);
    f.router
        .recv_packet(&packet, &Bytes::from_slice(&f.env, b"proof"), &10);

    assert_eq!(f.transfer.balance_of(&receiver, &xlm(&f.env)), 500);
    assert!(f.router.acknowledgement(&f.source_client_id, &1).is_some());
}


#[test]
fn acknowledge_packet_with_error_ack_refunds_sender() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &1_000);
    let seq = f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &400,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(f.transfer.balance_of(&sender, &xlm(&f.env)), 600);
    assert_eq!(f.transfer.balance_of(&f.transfer_addr, &xlm(&f.env)), 400);

    let packet = stellar_ibc_router::Packet {
        sequence: seq,
        source_client: f.source_client_id.clone(),
        dest_client: f.counterparty_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![
            &f.env,
            stellar_ibc_router::Payload {
                source_port: String::from_str(&f.env, "transfer"),
                dest_port: String::from_str(&f.env, "transfer"),
                version: String::from_str(&f.env, "ics20-2"),
                encoding: String::from_str(&f.env, "xdr"),
                value: {
                    let pkt = FungibleTokenPacketData {
                        token: Token {
                            denom: xlm(&f.env),
                            amount: 400,
                        },
                        sender: address_to_string(&f.env, &sender),
                        receiver: String::from_str(&f.env, "cosmos1abc"),
                        memo: String::from_str(&f.env, ""),
                    };
                    pkt.to_xdr(&f.env)
                },
            },
        ],
    };

    let err_ack = Bytes::from_slice(&f.env, b"\xff\xff");
    f.router.acknowledge_packet(
        &packet,
        &vec![&f.env, err_ack],
        &Bytes::from_slice(&f.env, b"proof"),
        &10,
    );

    assert_eq!(f.transfer.balance_of(&sender, &xlm(&f.env)), 1_000);
    assert_eq!(f.transfer.balance_of(&f.transfer_addr, &xlm(&f.env)), 0);
}

#[test]
fn acknowledge_packet_with_success_ack_leaves_escrow_released() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &1_000);
    let seq = f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &400,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );

    let packet = stellar_ibc_router::Packet {
        sequence: seq,
        source_client: f.source_client_id.clone(),
        dest_client: f.counterparty_client_id.clone(),
        timeout_timestamp: 2_000,
        payloads: vec![
            &f.env,
            stellar_ibc_router::Payload {
                source_port: String::from_str(&f.env, "transfer"),
                dest_port: String::from_str(&f.env, "transfer"),
                version: String::from_str(&f.env, "ics20-2"),
                encoding: String::from_str(&f.env, "xdr"),
                value: {
                    let pkt = FungibleTokenPacketData {
                        token: Token {
                            denom: xlm(&f.env),
                            amount: 400,
                        },
                        sender: address_to_string(&f.env, &sender),
                        receiver: String::from_str(&f.env, "cosmos1abc"),
                        memo: String::from_str(&f.env, ""),
                    };
                    pkt.to_xdr(&f.env)
                },
            },
        ],
    };

    let success_ack = Bytes::from_slice(&f.env, &[SUCCESS_ACK_BYTE]);
    f.router.acknowledge_packet(
        &packet,
        &vec![&f.env, success_ack],
        &Bytes::from_slice(&f.env, b"proof"),
        &10,
    );

    assert_eq!(f.transfer.balance_of(&sender, &xlm(&f.env)), 600);
    assert_eq!(f.transfer.balance_of(&f.transfer_addr, &xlm(&f.env)), 400);
}


#[test]
fn mint_and_balance_of_round_trip() {
    let f = setup();
    let who = Address::generate(&f.env);
    assert_eq!(f.transfer.balance_of(&who, &xlm(&f.env)), 0);
    f.transfer.mint(&who, &xlm(&f.env), &123);
    assert_eq!(f.transfer.balance_of(&who, &xlm(&f.env)), 123);
}

#[test]
fn set_rate_limit_stores_cap_and_starts_at_zero_usage() {
    let f = setup();
    let _ = &f.transfer_admin;
    f.transfer.set_rate_limit(&xlm(&f.env), &500);
    assert_eq!(f.transfer.daily_cap(&xlm(&f.env)), Some(500));
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 0);
}

#[test]
fn initiate_transfer_under_cap_accumulates_usage() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &1_000);
    f.transfer.set_rate_limit(&xlm(&f.env), &500);

    f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &200,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &150,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 350);
}

#[test]
fn initiate_transfer_over_cap_is_rejected() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &10_000);
    f.transfer.set_rate_limit(&xlm(&f.env), &500);

    f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &400,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    let result = f.transfer.try_initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &200,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(result, Err(Ok(Error::DailyLimitExceeded.into())));
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 400);
}

#[test]
fn rate_limit_resets_after_day_rolls_over() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &10_000);
    f.transfer.set_rate_limit(&xlm(&f.env), &500);

    f.env.ledger().set_timestamp(86_400);
    f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &500,
        &String::from_str(&f.env, "cosmos1abc"),
        &172_800,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 500);

    f.env.ledger().set_timestamp(172_800);
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 0);

    f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &500,
        &String::from_str(&f.env, "cosmos1abc"),
        &200_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 500);
}

#[test]
fn rate_limit_unset_denom_is_unlimited() {
    let f = setup();
    let sender = Address::generate(&f.env);
    f.transfer.mint(&sender, &xlm(&f.env), &10_000);
    f.transfer.initiate_transfer(
        &sender,
        &f.source_client_id,
        &xlm(&f.env),
        &9_999,
        &String::from_str(&f.env, "cosmos1abc"),
        &2_000,
        &String::from_str(&f.env, ""),
    );
    assert_eq!(f.transfer.daily_usage(&xlm(&f.env)), 0);
    assert_eq!(f.transfer.daily_cap(&xlm(&f.env)), None);
}
