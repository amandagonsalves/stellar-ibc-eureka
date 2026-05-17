use ibc::core::{
    client::context::{
        ClientExecutionContext, ClientValidationContext,
        client_state::{ClientStateCommon, ClientStateExecution, ClientStateValidation},
        consensus_state::ConsensusState,
    },
    client::types::{error::ClientError, Height, Status},
    commitment_types::commitment::{
        CommitmentPrefix, CommitmentProofBytes, CommitmentRoot,
    },
    host::types::{
        error::HostError,
        identifiers::{ClientId, ClientType},
        path::{ClientConsensusStatePath, ClientStatePath, Path, PathBytes},
    },
    primitives::{proto::Any, Timestamp},
};

use crate::context::{StellarIbcContext, storage::SorobanStorage};

#[derive(Clone, Debug)]
pub struct MockClientState;

impl TryFrom<Any> for MockClientState {
    type Error = ClientError;
    fn try_from(_value: Any) -> Result<Self, Self::Error> {
        unimplemented!("MockClientState::try_from")
    }
}

impl From<MockClientState> for Any {
    fn from(_value: MockClientState) -> Self {
        unimplemented!("MockClientState::into Any")
    }
}

impl ClientStateCommon for MockClientState {
    fn verify_consensus_state(
        &self,
        _consensus_state: Any,
        _host_timestamp: &Timestamp,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn client_type(&self) -> ClientType {
        unimplemented!()
    }

    fn latest_height(&self) -> Height {
        unimplemented!()
    }

    fn validate_proof_height(&self, _proof_height: Height) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn verify_upgrade_client(
        &self,
        _upgraded_client_state: Any,
        _upgraded_consensus_state: Any,
        _proof_upgrade_client: CommitmentProofBytes,
        _proof_upgrade_consensus_state: CommitmentProofBytes,
        _root: &CommitmentRoot,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn serialize_path(&self, _path: Path) -> Result<PathBytes, ClientError> {
        unimplemented!()
    }

    fn verify_membership_raw(
        &self,
        _prefix: &CommitmentPrefix,
        _proof: &CommitmentProofBytes,
        _root: &CommitmentRoot,
        _path: PathBytes,
        _value: Vec<u8>,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn verify_non_membership_raw(
        &self,
        _prefix: &CommitmentPrefix,
        _proof: &CommitmentProofBytes,
        _root: &CommitmentRoot,
        _path: PathBytes,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }
}

impl<V: ClientValidationContext> ClientStateValidation<V> for MockClientState {
    fn verify_client_message(
        &self,
        _ctx: &V,
        _client_id: &ClientId,
        _client_message: Any,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn check_for_misbehaviour(
        &self,
        _ctx: &V,
        _client_id: &ClientId,
        _client_message: Any,
    ) -> Result<bool, ClientError> {
        unimplemented!()
    }

    fn status(&self, _ctx: &V, _client_id: &ClientId) -> Result<Status, ClientError> {
        unimplemented!()
    }

    fn check_substitute(
        &self,
        _ctx: &V,
        _substitute_client_state: Any,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }
}

impl<E: ClientExecutionContext> ClientStateExecution<E> for MockClientState {
    fn initialise(
        &self,
        _ctx: &mut E,
        _client_id: &ClientId,
        _consensus_state: Any,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn update_state(
        &self,
        _ctx: &mut E,
        _client_id: &ClientId,
        _header: Any,
    ) -> Result<Vec<Height>, ClientError> {
        unimplemented!()
    }

    fn update_state_on_misbehaviour(
        &self,
        _ctx: &mut E,
        _client_id: &ClientId,
        _client_message: Any,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }

    fn update_state_on_upgrade(
        &self,
        _ctx: &mut E,
        _client_id: &ClientId,
        _upgraded_client_state: Any,
        _upgraded_consensus_state: Any,
    ) -> Result<Height, ClientError> {
        unimplemented!()
    }

    fn update_on_recovery(
        &self,
        _ctx: &mut E,
        _subject_client_id: &ClientId,
        _substitute_client_state: Any,
        _substitute_consensus_state: Any,
    ) -> Result<(), ClientError> {
        unimplemented!()
    }
}

// ---------------------------------------------------------------------------
// MockConsensusState
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct MockConsensusState;

impl TryFrom<Any> for MockConsensusState {
    type Error = ClientError;
    fn try_from(_value: Any) -> Result<Self, Self::Error> {
        unimplemented!("MockConsensusState::try_from")
    }
}

impl From<MockConsensusState> for Any {
    fn from(_value: MockConsensusState) -> Self {
        unimplemented!("MockConsensusState::into Any")
    }
}

impl ConsensusState for MockConsensusState {
    fn root(&self) -> &CommitmentRoot {
        unimplemented!()
    }

    fn timestamp(&self) -> Result<Timestamp, ClientError> {
        unimplemented!()
    }
}

impl<S: SorobanStorage> ClientValidationContext for StellarIbcContext<S> {
    type ClientStateRef = MockClientState;
    type ConsensusStateRef = MockConsensusState;

    fn client_state(&self, _client_id: &ClientId) -> Result<Self::ClientStateRef, HostError> {
        Err(HostError::missing_state("client state: not implemented"))
    }

    fn consensus_state(
        &self,
        _client_cons_state_path: &ClientConsensusStatePath,
    ) -> Result<Self::ConsensusStateRef, HostError> {
        Err(HostError::missing_state("consensus state: not implemented"))
    }

    fn client_update_meta(
        &self,
        _client_id: &ClientId,
        _height: &Height,
    ) -> Result<(Timestamp, Height), HostError> {
        Err(HostError::missing_state("client update meta: not implemented"))
    }
}

impl<S: SorobanStorage> ClientExecutionContext for StellarIbcContext<S> {
    type ClientStateMut = MockClientState;

    fn store_client_state(
        &mut self,
        _client_state_path: ClientStatePath,
        _client_state: Self::ClientStateRef,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("store_client_state: not implemented"))
    }

    fn store_consensus_state(
        &mut self,
        _consensus_state_path: ClientConsensusStatePath,
        _consensus_state: Self::ConsensusStateRef,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("store_consensus_state: not implemented"))
    }

    fn delete_consensus_state(
        &mut self,
        _consensus_state_path: ClientConsensusStatePath,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("delete_consensus_state: not implemented"))
    }

    fn store_update_meta(
        &mut self,
        _client_id: ClientId,
        _height: Height,
        _host_timestamp: Timestamp,
        _host_height: Height,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("store_update_meta: not implemented"))
    }

    fn delete_update_meta(
        &mut self,
        _client_id: ClientId,
        _height: Height,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("delete_update_meta: not implemented"))
    }
}