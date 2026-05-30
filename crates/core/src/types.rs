use soroban_client::xdr::ScVal;

#[derive(Debug, Clone)]
pub struct LedgerData {
    pub sequence: u32,
    pub header_xdr: Vec<u8>,
    pub metadata_xdr: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct SubmittedTx {
    pub hash: String,
    pub return_value: Option<ScVal>,
}
