#[tokio::main]
async fn main() {
    stellar_api::start().await;
}
