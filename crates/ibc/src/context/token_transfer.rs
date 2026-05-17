use core::marker::PhantomData;

use ibc::apps::transfer::context::{
    TokenTransferExecutionContext, TokenTransferValidationContext,
};
use ibc::apps::transfer::types::{Memo, PrefixedCoin, PrefixedDenom};
use ibc::core::host::types::{
    error::HostError,
    identifiers::{ChannelId, PortId},
};
use ibc::core::primitives::Signer;

use crate::context::storage::SorobanStorage;

pub struct TokenTransferContext<S>(PhantomData<S>);

impl<S: SorobanStorage> TokenTransferValidationContext for TokenTransferContext<S> {
    type AccountId = String;

    fn sender_account(&self, _sender: &Signer) -> Result<Self::AccountId, HostError> {
        Err(HostError::missing_state("sender_account: not implemented"))
    }

    fn receiver_account(&self, _receiver: &Signer) -> Result<Self::AccountId, HostError> {
        Err(HostError::missing_state("receiver_account: not implemented"))
    }

    fn get_port(&self) -> Result<PortId, HostError> {
        Err(HostError::missing_state("get_port: not implemented"))
    }

    fn can_send_coins(&self) -> Result<(), HostError> {
        Err(HostError::invalid_state("can_send_coins: not implemented"))
    }

    fn can_receive_coins(&self) -> Result<(), HostError> {
        Err(HostError::invalid_state("can_receive_coins: not implemented"))
    }

    fn escrow_coins_validate(
        &self,
        _from_account: &Self::AccountId,
        _port_id: &PortId,
        _channel_id: &ChannelId,
        _coin: &PrefixedCoin,
        _memo: &Memo,
    ) -> Result<(), HostError> {
        Err(HostError::invalid_state("escrow_coins_validate: not implemented"))
    }

    fn unescrow_coins_validate(
        &self,
        _to_account: &Self::AccountId,
        _port_id: &PortId,
        _channel_id: &ChannelId,
        _coin: &PrefixedCoin,
    ) -> Result<(), HostError> {
        Err(HostError::invalid_state("unescrow_coins_validate: not implemented"))
    }

    fn mint_coins_validate(
        &self,
        _account: &Self::AccountId,
        _coin: &PrefixedCoin,
    ) -> Result<(), HostError> {
        Err(HostError::invalid_state("mint_coins_validate: not implemented"))
    }

    fn burn_coins_validate(
        &self,
        _account: &Self::AccountId,
        _coin: &PrefixedCoin,
        _memo: &Memo,
    ) -> Result<(), HostError> {
        Err(HostError::invalid_state("burn_coins_validate: not implemented"))
    }

    fn denom_hash_string(&self, _denom: &PrefixedDenom) -> Option<String> {
        None
    }
}

impl<S: SorobanStorage> TokenTransferExecutionContext for TokenTransferContext<S> {
    fn escrow_coins_execute(
        &mut self,
        _from_account: &Self::AccountId,
        _port_id: &PortId,
        _channel_id: &ChannelId,
        _coin: &PrefixedCoin,
        _memo: &Memo,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("escrow_coins_execute: not implemented"))
    }

    fn unescrow_coins_execute(
        &mut self,
        _to_account: &Self::AccountId,
        _port_id: &PortId,
        _channel_id: &ChannelId,
        _coin: &PrefixedCoin,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("unescrow_coins_execute: not implemented"))
    }

    fn mint_coins_execute(
        &mut self,
        _account: &Self::AccountId,
        _coin: &PrefixedCoin,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("mint_coins_execute: not implemented"))
    }

    fn burn_coins_execute(
        &mut self,
        _account: &Self::AccountId,
        _coin: &PrefixedCoin,
        _memo: &Memo,
    ) -> Result<(), HostError> {
        Err(HostError::failed_to_store("burn_coins_execute: not implemented"))
    }
}
