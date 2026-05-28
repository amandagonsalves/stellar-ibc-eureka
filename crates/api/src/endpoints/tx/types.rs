use serde::Serialize;

#[derive(Serialize)]
pub struct SubmitSignedTxResponse {
    pub account_id: String,
}

#[derive(Serialize)]
pub struct SignTxResponse {
    pub account_id: String,
}

#[derive(Serialize)]
pub struct GetUnsignedTxResponse {
    pub account_id: String,
}

#[derive(Serialize)]
pub struct GetSignedTxResponse {
    pub account_id: String,
}
