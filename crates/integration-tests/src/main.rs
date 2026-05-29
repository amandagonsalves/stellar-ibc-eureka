mod gateway_tests;
mod pb;

use soroban_client::{
    account::{Account, AccountBehavior},
    keypair::{Keypair, KeypairBehavior},
    network::{NetworkPassphrase, Networks},
    operation::{self, Operation},
    transaction::{TransactionBehavior, TransactionBuilder, TransactionBuilderBehavior},
    xdr::{LedgerHeader, LedgerKey, LedgerKeyAccount, Limits, ReadXdr, StellarValueExt, WriteXdr},
    Options, Server,
};
use stellar_ibc_core::rpc::RpcClient;

const TESTNET_URL: &str = "https://soroban-testnet.stellar.org";

pub fn pass(label: &str) {
    println!("PASS  {label}");
}

pub fn fail(label: &str, err: impl std::fmt::Display) {
    println!("FAIL  {label}: {err}");
}

async fn test_get_known_account(client: &RpcClient, kp: &Keypair) {
    let label = "get_ledger_entry: funded account returns data";
    let account_id = kp.xdr_account_id();
    let key = LedgerKey::Account(LedgerKeyAccount { account_id });
    let key_xdr = key.to_xdr(Limits::none()).unwrap();

    match client.get_ledger_entry(&key_xdr).await {
        Ok(Some(data)) => {
            use soroban_client::xdr::LedgerEntryData;
            match LedgerEntryData::from_xdr(&data, Limits::none()) {
                Ok(LedgerEntryData::Account(_)) => pass(label),
                Ok(other) => fail(label, format!("unexpected entry type: {other:?}")),
                Err(e) => fail(label, format!("XDR decode failed: {e}")),
            }
        }
        Ok(None) => fail(label, "entry not found — account may not be funded yet"),
        Err(e) => fail(label, e),
    }
}

async fn test_get_missing_key(client: &RpcClient) {
    let label = "get_ledger_entry: non-existent key returns None";
    let random_kp = Keypair::random().unwrap();
    let key = LedgerKey::Account(LedgerKeyAccount {
        account_id: random_kp.xdr_account_id(),
    });
    let key_xdr = key.to_xdr(Limits::none()).unwrap();

    match client.get_ledger_entry(&key_xdr).await {
        Ok(None) => pass(label),
        Ok(Some(_)) => fail(label, "expected None but got data"),
        Err(e) => fail(label, e),
    }
}

async fn test_get_ledger(client: &RpcClient) {
    let label = "get_ledger: returns LedgerHeader XDR for a recent sequence";

    let seq = match client.get_latest_ledger().await {
        Ok(s) => s.saturating_sub(2),
        Err(e) => {
            fail(label, format!("get_latest_ledger failed: {e}"));
            return;
        }
    };

    match client.get_ledger(seq).await {
        Ok(ledger) => {
            if ledger.header_xdr.is_empty() {
                fail(label, "header_xdr is empty");
                return;
            }
            match LedgerHeader::from_xdr(&ledger.header_xdr, Limits::none()) {
                Ok(h) if h.ledger_seq == seq => pass(&format!("{label} (seq: {seq})")),
                Ok(h) => fail(label, format!("sequence mismatch: got {}", h.ledger_seq)),
                Err(e) => fail(label, format!("LedgerHeader XDR decode failed: {e}")),
            }
        }
        Err(e) => fail(label, e),
    }
}

async fn test_get_ledger_scp_signature(client: &RpcClient) {
    let label = "get_ledger: LedgerHeader contains SCP signature";

    let seq = match client.get_latest_ledger().await {
        Ok(s) => s.saturating_sub(2),
        Err(e) => {
            fail(label, format!("get_latest_ledger failed: {e}"));
            return;
        }
    };

    let ledger = match client.get_ledger(seq).await {
        Ok(l) => l,
        Err(e) => {
            fail(label, e);
            return;
        }
    };

    let header = match LedgerHeader::from_xdr(&ledger.header_xdr, Limits::none()) {
        Ok(h) => h,
        Err(e) => {
            fail(label, format!("LedgerHeader XDR decode failed: {e}"));
            return;
        }
    };

    match header.scp_value.ext {
        StellarValueExt::Signed(sig) => {
            let sig_bytes = sig.signature.to_vec();
            if sig_bytes.is_empty() {
                fail(label, "SCP signature is empty");
            } else {
                pass(&format!(
                    "{label} (sig len: {}, node_id present)",
                    sig_bytes.len()
                ));
            }
        }
        StellarValueExt::Basic => {
            fail(label, "scp_value.ext is Basic — no signature present");
        }
    }
}

async fn test_get_ledger_unknown_sequence(client: &RpcClient) {
    let label = "get_ledger: unknown sequence returns error";

    match client.get_ledger(u32::MAX).await {
        Err(_) => pass(label),
        Ok(_) => fail(label, "expected error for sequence u32::MAX but got Ok"),
    }
}

async fn test_submit_payment(client: &RpcClient, source_kp: &Keypair, server: &Server) {
    let label = "submit_and_wait: XLM payment confirms on-chain";

    let pk = source_kp.public_key();
    let account_data = match server.get_account(&pk).await {
        Ok(a) => a,
        Err(e) => {
            fail(label, format!("get_account failed: {e}"));
            return;
        }
    };
    let mut source_account = Account::new(&pk, &account_data.sequence_number()).unwrap();

    let dest_kp = Keypair::random().unwrap();
    let op = Operation::new()
        .create_account(&dest_kp.public_key(), operation::ONE)
        .unwrap();

    let mut builder = TransactionBuilder::new(&mut source_account, Networks::testnet(), None);
    builder.fee(1000u32);
    builder.add_operation(op);
    let mut tx = builder.build();
    tx.sign(std::slice::from_ref(source_kp));

    let envelope = match tx.to_envelope() {
        Ok(e) => e,
        Err(e) => {
            fail(label, format!("to_envelope failed: {e:?}"));
            return;
        }
    };
    let xdr_bytes = match envelope.to_xdr(Limits::none()) {
        Ok(b) => b,
        Err(e) => {
            fail(label, format!("XDR encode failed: {e}"));
            return;
        }
    };

    print!("  submitting transaction (waiting up to 30s)...");
    use std::io::Write;
    std::io::stdout().flush().ok();

    match client.submit_and_wait(&xdr_bytes).await {
        Ok(hash) => {
            println!(" done.");
            pass(&format!("{label} (hash: {hash})"));
        }
        Err(e) => {
            println!(" failed.");
            fail(label, e);
        }
    }
}

#[tokio::main]
async fn main() {
    let client = RpcClient::new(TESTNET_URL).expect("failed to create RpcClient");
    let server = Server::new(TESTNET_URL, Options::default()).expect("failed to create Server");

    println!("Funding a fresh testnet account via friendbot...");
    let source_kp = Keypair::random().unwrap();
    match server.request_airdrop(&source_kp.public_key()).await {
        Ok(_) => println!("  funded: {}", source_kp.public_key()),
        Err(e) => println!("  friendbot failed: {e} — network tests may fail"),
    }

    println!("\n--- get_ledger_entry ---");
    test_get_known_account(&client, &source_kp).await;
    test_get_missing_key(&client).await;

    println!("\n--- get_ledger ---");
    test_get_ledger(&client).await;
    test_get_ledger_scp_signature(&client).await;
    test_get_ledger_unknown_sequence(&client).await;

    println!("\n--- submit_and_wait ---");
    test_submit_payment(&client, &source_kp, &server).await;

    let addr = gateway_tests::gateway_addr();
    gateway_tests::run_all(&addr).await;

    println!("\nDone.");
}
