use std::sync::Arc;

use soroban_client::{Options, Server};

#[derive(Clone)]
pub struct RpcClient {
    pub server: Arc<Server>,
}

impl RpcClient {
    pub fn new(rpc_url: &str) -> anyhow::Result<Self> {
        let server = Server::new(
            rpc_url,
            Options {
                allow_http: true,
                ..Default::default()
            },
        )?;
        Ok(Self {
            server: Arc::new(server),
        })
    }

    pub async fn latest_ledger_sequence(&self) -> anyhow::Result<u32> {
        let info = self.server.get_latest_ledger().await?;

        Ok(info.sequence)
    }

    pub async fn get_ledger_entry(&self, _key: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
        Err(anyhow::anyhow!("not implemented"))
    }

    pub async fn submit_and_wait(&self, _tx_xdr: &[u8]) -> anyhow::Result<String> {
        Err(anyhow::anyhow!("not implemented"))
    }
}
