// CosmWasm entry points for the Stellar IBC light client.
//
// The pattern mirrors references/cosmwasm-ibc/ibc-clients/ics07-tendermint/src/entrypoint.rs.
// Full implementation follows in A-4 (ClientState trait impls) and A-5 (wiring).

use cosmwasm_std::{entry_point, Binary, Deps, DepsMut, Env, MessageInfo, Response};
use ibc_client_cw::context::Context;
use ibc_client_cw::types::{ContractError, InstantiateMsg, QueryMsg, SudoMsg};

use crate::client_type::StellarClient;

mod client_type;

pub type StellarContext<'a> = Context<'a, StellarClient>;

#[entry_point]
pub fn instantiate(
    deps: DepsMut<'_>,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let mut ctx = StellarContext::new_mut(deps, env)?;
    let data = ctx.instantiate(msg)?;
    Ok(Response::default().set_data(data))
}

#[entry_point]
pub fn sudo(deps: DepsMut<'_>, env: Env, msg: SudoMsg) -> Result<Response, ContractError> {
    let mut ctx = StellarContext::new_mut(deps, env)?;
    let data = ctx.sudo(msg)?;
    Ok(Response::default().set_data(data))
}

#[entry_point]
pub fn query(deps: Deps<'_>, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    let ctx = StellarContext::new_ref(deps, env)?;
    ctx.query(msg)
}
