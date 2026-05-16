use ibc_client_cw::api::ClientType;
use stellar_types::{StellarClientState, StellarConsensusState};

pub struct StellarClient;

impl<'a> ClientType<'a> for StellarClient {
    type ClientState = StellarClientState;
    type ConsensusState = StellarConsensusState;
}
