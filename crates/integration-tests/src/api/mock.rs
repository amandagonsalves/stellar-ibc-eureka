use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::net::TcpListener;

use stellar_api::rpc::RpcClient;
use stellar_api::runner::router;
use stellar_api::AppState;

pub struct ApiTest {
    base_url: String,
    http: reqwest::Client,
}

impl ApiTest {
    pub async fn start(ibc_contract_id: &str, transfer_contract_id: &str) -> Self {
        let rpc = RpcClient::new("http://127.0.0.1:1").expect("rpc client");
        let state = Arc::new(AppState::new(
            rpc,
            ibc_contract_id.to_string(),
            transfer_contract_id.to_string(),
            "Test SDF Network ; September 2015".to_string(),
        ));

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, router(state)).await.unwrap();
        });

        let test = Self {
            base_url: format!("http://{addr}"),
            http: reqwest::Client::new(),
        };
        test.wait_ready().await;
        test
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn wait_ready(&self) {
        for _ in 0..100 {
            if self
                .http
                .get(self.url("/api-docs/openapi.json"))
                .send()
                .await
                .is_ok()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("api did not start serving at {}", self.base_url);
    }

    pub async fn get(&self, path: &str) -> (u16, String) {
        self.send(self.http.get(self.url(path))).await
    }

    pub async fn post(&self, path: &str, body: Value) -> (u16, String) {
        self.send(self.http.post(self.url(path)).json(&body)).await
    }

    pub async fn get_json(&self, path: &str) -> Value {
        let (_, body) = self.get(path).await;
        serde_json::from_str(&body).expect("json body")
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> (u16, String) {
        let resp = req.send().await.expect("request");
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        (status, body)
    }
}

pub fn sample_contract_id() -> String {
    format!("{}", stellar_strkey::Contract([7u8; 32]))
}
