use crate::error::Error;
use ibc::core::host::types::{
    identifiers::ClientId,
    path::{ClientStatePath, Path},
};
use stellar_ibc_core::storage::SorobanKey;

pub fn client_state_key(client_id: &ClientId) -> SorobanKey {
    let path = Path::ClientState(ClientStatePath(client_id.clone()));

    ibc_key(path.to_string()).expect("Creating a key for the client state shouldn't fail")
}

pub fn ibc_key(_path: impl AsRef<str>) -> Result<SorobanKey, Error> {
    Ok(SorobanKey {})
}
