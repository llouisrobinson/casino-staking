use std::collections::HashMap;

use cosmwasm_schema::cw_serde;

use cosmwasm_std::{Addr, Decimal, Deps, Response, StdResult, Storage, Uint128};
use cw_storage_plus::{Item, Map};

use crate::msg::Unlock;

#[cw_serde]
pub struct Config {
    pub owner: Addr,

    pub kart_denom: String,

    pub unlock_time: u64,

    pub distribution_schedule: Vec<(u64, u64, Uint128, String)>,
}

#[cw_serde]
pub struct State {
    pub total_staker: u64,
    pub total_stake_amount: Uint128,
    pub last_distributed: u64,
    pub reward_index: HashMap<String, Decimal>,
    pub reward_distributed: HashMap<String, Uint128>,
}

#[cw_serde]
pub struct StakerInfo {
    pub stake_amount: Uint128,
    pub pending_reward: HashMap<String, Uint128>,
    pub reward_index: HashMap<String, Decimal>,
    pub reward_claimed: HashMap<String, Uint128>,
    pub unlock: Option<Vec<Unlock>>,
}

pub const CONFIG: Item<Config> = Item::new("config");
pub const STATE: Item<State> = Item::new("state");
pub const USER_STAKING: Map<&str, StakerInfo> = Map::new("user_info");

pub fn user_staking(deps: Deps, sender: &str) -> StdResult<StakerInfo> {
    let staking_info = USER_STAKING.may_load(deps.storage, sender).unwrap();

    match staking_info {
        Some(staking_info) => Ok(staking_info),
        None => Ok(StakerInfo {
            stake_amount: Uint128::zero(),
            pending_reward: HashMap::new(),
            reward_index: HashMap::new(),
            reward_claimed: HashMap::new(),
            unlock: None,
        }),
    }
}

pub fn store_user_staking(
    storage: &mut dyn Storage,
    owner: &str,
    staker_info: &StakerInfo,
) -> StdResult<Response> {
    USER_STAKING.save(storage, owner, staker_info)?;
    Ok(Response::new())
}

pub fn remove_user_staking(storage: &mut dyn Storage, owner: &str) -> StdResult<Response> {
    USER_STAKING.remove(storage, owner);
    Ok(Response::new())
}

pub fn load_state(deps: Deps) -> StdResult<State> {
    let state_info = STATE.load(deps.storage).unwrap();
    return Ok(state_info);
}

pub fn store_state(storage: &mut dyn Storage, state: &State) -> StdResult<Response> {
    STATE.save(storage, state).unwrap();
    return Ok(Response::new());
}
