use std::collections::HashMap;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakerInfoResponse, StateResponse, Unlock,
};
use crate::state::{
    load_state, remove_user_staking, store_state, store_user_staking, user_staking, Config,
    StakerInfo, State, CONFIG, STATE,
};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Addr, BankMsg, Binary, Coin, Decimal, Deps, DepsMut, Env, MessageInfo,
    Response, StdResult, Uint128,
};
use cw2::set_contract_version;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:kartel_staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CONFIG.save(
        deps.storage,
        &Config {
            owner: info.sender,
            kart_denom: msg.kart_denom,
            unlock_time: msg.unlock_time,
            distribution_schedule: msg.distribution_schedule,
        },
    )?;

    STATE.save(
        deps.storage,
        &State {
            total_staker: 0,
            total_stake_amount: Uint128::zero(),
            last_distributed: 0,
            reward_index: HashMap::new(),
            reward_distributed: HashMap::new(),
        },
    )?;

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Stake {} => stake(deps, env, info),
        ExecuteMsg::Unstake { amount } => unstake(deps, env, info.sender, amount),
        ExecuteMsg::Claim {} => claim_reward(deps, env, info),
        ExecuteMsg::Withdraw { id } => withdraw(deps, env, info, id),
        ExecuteMsg::Unlock { amount , denom} => unlock(deps, env, info, amount, denom),
        ExecuteMsg::SetDistribution {
            reward_denom,
            start_date,
            end_date,
            amount,
        } => set_distribution_schedule(deps, env, info, reward_denom, start_date, end_date, amount),
        ExecuteMsg::UpdateConfig { config } => update_config(deps, env, info, config),
    }
}

pub fn stake(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let mut staker_info: StakerInfo = user_staking(deps.as_ref(), info.sender.as_str())?;

    let mut state: State = load_state(deps.as_ref())?;

    // Transfer tokens from sender to the contract
    if info.funds.len() != 1 || info.funds[0].denom != config.kart_denom {
        return Err(ContractError::UnsupportedToken {});
    }

    let amount = info
        .funds
        .iter()
        .find(|coin| coin.denom == config.kart_denom)
        .map(|coin| coin.amount)
        .unwrap_or(Uint128::zero());

    if amount == Uint128::zero() {
        return Err(ContractError::InvalidAmount {});
    }

    if staker_info.stake_amount == Uint128::zero() {
        state.total_staker += 1;
    }

    compute_reward(&config, &mut state, env.block.time.seconds());

    compute_staker_reward(&state, &mut staker_info)?;
    // Increase bond_amount
    increase_stake_amount(&mut state, &mut staker_info, amount);

    store_user_staking(deps.storage, info.sender.as_str(), &staker_info)?;
    store_state(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "stake"),
        ("owner", info.sender.as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

pub fn unstake(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let mut staker_info: StakerInfo = user_staking(deps.as_ref(), sender.as_str())?;

    if staker_info.stake_amount < amount {
        return Err(ContractError::InsufficientToken {});
    }

    let mut state: State = load_state(deps.as_ref())?;
    compute_reward(&config, &mut state, env.block.time.seconds());

    compute_staker_reward(&state, &mut staker_info)?;
    // decrease bond_amount
    decrease_stake_amount(
        &mut state,
        &mut staker_info,
        amount,
        env.block.time.seconds() + config.unlock_time,
    );

    if staker_info.stake_amount == Uint128::zero() {
        state.total_staker -= 1;
    }

    store_user_staking(deps.storage, sender.as_str(), &staker_info)?;
    store_state(deps.storage, &state)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "unstake"),
        ("owner", sender.to_string().as_str()),
        ("amount", amount.to_string().as_str()),
    ]))
}

pub fn withdraw(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: usize,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut staker_info: StakerInfo = user_staking(deps.as_ref(), info.sender.as_str())?;

    let amount: Uint128;
    let unlock_at: u64;

    match &mut staker_info.unlock {
        Some(unlock) => {
            if id < unlock.len() {
                amount = unlock
                    .get(id)
                    .unwrap_or(&Unlock {
                        amount: Uint128::zero(),
                        at: 0,
                    })
                    .amount;
                unlock_at = unlock
                    .get(id)
                    .unwrap_or(&Unlock {
                        amount: Uint128::zero(),
                        at: 0,
                    })
                    .at;

                if env.block.time.seconds() < unlock_at {
                    return Err(ContractError::CustomError {
                        msg: "withdraw after pending period".to_string(),
                    });
                }

                unlock.remove(id);

                if unlock.len() == 0 && staker_info.stake_amount == Uint128::zero() {
                    claim_reward(deps.branch(), env.clone(), info.clone())?;
                    remove_user_staking(deps.storage, info.sender.as_str())?;
                }

                if unlock.len() == 0 {
                    staker_info.unlock = None;
                }
            } else {
                return Err(ContractError::CustomError {
                    msg: "invalid index".to_string(),
                });
            }
        }
        None => {
            return Err(ContractError::CustomError {
                msg: "dont have any pending unstake".to_string(),
            })
        }
    }

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom: config.kart_denom,
                amount,
            }],
        })
        .add_attributes(vec![
            ("action", "withdraw"),
            ("owner", info.sender.to_string().as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

// withdraw rewards to executor
pub fn claim_reward(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config: Config = CONFIG.load(deps.storage)?;
    let mut staker_info: StakerInfo = user_staking(deps.as_ref(), info.sender.as_str())?;

    let mut state: State = load_state(deps.as_ref())?;

    // Compute global reward & staker reward
    compute_reward(&config, &mut state, env.block.time.seconds());

    compute_staker_reward(&state, &mut staker_info)?;

    let mut reward: Vec<Coin> = vec![];

    for (denom, reward_amount) in staker_info.pending_reward {
        staker_info
            .reward_claimed
            .entry(denom.clone())
            .and_modify(|e| *e += reward_amount)
            .or_insert(reward_amount);
        reward.push(Coin::new(reward_amount.u128(), denom));
    }

    staker_info.pending_reward = HashMap::new();

    store_user_staking(deps.storage, info.sender.as_str(), &staker_info)?;
    store_state(deps.storage, &state)?;

    // Store updated state

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: reward,
        })
        .add_attributes(vec![
            ("action", "claim_reward"),
            ("owner", info.sender.as_str()),
        ]))
}

pub fn unlock(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    amount: Uint128,
    denom: String
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    Ok(Response::new()
        .add_message(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: vec![Coin {
                denom,
                amount,
            }],
        })
        .add_attributes(vec![
            ("action", "unlock"),
            ("owner", info.sender.to_string().as_str()),
            ("amount", amount.to_string().as_str()),
        ]))
}

pub fn update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    new_config: Config,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

pub fn set_distribution_schedule(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    reward_denom: String,
    start_date: u64,
    end_date: u64,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    let mut state: State = load_state(deps.as_ref())?;

    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    config
        .distribution_schedule
        .push((start_date, end_date, amount, reward_denom.clone()));
    state
        .reward_distributed
        .entry(reward_denom)
        .and_modify(|e| *e += amount)
        .or_insert(amount);

    store_state(deps.storage, &state)?;

    CONFIG.save(deps.storage, &config)?;

    Ok(Response::new().add_attributes(vec![("action", "update_config")]))
}

fn compute_reward(config: &Config, state: &mut State, block_time: u64) {
    if state.total_stake_amount.is_zero() {
        state.last_distributed = block_time;
        return;
    };

    for s in config.distribution_schedule.iter() {
        if s.0 > block_time || s.1 < state.last_distributed {
            continue;
        }

        let passed_time =
            std::cmp::min(s.1, block_time) - std::cmp::max(s.0, state.last_distributed);

        let time = s.1 - s.0;
        let distribution_amount_per_second = Decimal::from_ratio(s.2, time);

        let reward_index_delta = Decimal::from_ratio(
            distribution_amount_per_second * Uint128::from(passed_time as u128),
            state.total_stake_amount,
        );

        state
            .reward_index
            .entry(s.3.clone())
            .and_modify(|e| *e += reward_index_delta)
            .or_insert(reward_index_delta);
    }

    state.last_distributed = block_time;
}

fn compute_staker_reward(state: &State, staker_info: &mut StakerInfo) -> StdResult<()> {
    for (reward_denom, reward_index) in &state.reward_index {
        let pending_rewards = (staker_info.stake_amount * *reward_index).checked_sub(
            staker_info.stake_amount
                * *staker_info
                    .reward_index
                    .get(reward_denom)
                    .unwrap_or(&Decimal::zero()),
        )?;

        staker_info
            .reward_index
            .insert(reward_denom.clone(), *reward_index);
        staker_info
            .pending_reward
            .entry(reward_denom.clone())
            .and_modify(|e| *e += pending_rewards)
            .or_insert(pending_rewards);
    }

    Ok(())
}

fn increase_stake_amount(state: &mut State, staker_info: &mut StakerInfo, amount: Uint128) {
    state.total_stake_amount += amount;
    staker_info.stake_amount += amount;
}

fn decrease_stake_amount(
    state: &mut State,
    staker_info: &mut StakerInfo,
    amount: Uint128,
    unlock_at: u64,
) {
    state.total_stake_amount -= amount;
    staker_info.stake_amount -= amount;

    let unlock = Unlock {
        amount: amount,
        at: unlock_at,
    };

    match &mut staker_info.unlock {
        Some(unlocks) => unlocks.push(unlock),
        None => staker_info.unlock = Some(vec![unlock]),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::State { block_time } => to_json_binary(&query_state(deps, block_time)?),
        QueryMsg::StakerInfo { staker, block_time } => {
            to_json_binary(&query_staker_info(deps, staker, block_time)?)
        }
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;

    let resp = ConfigResponse {
        owner: config.owner.to_string(),
        kart_denom: config.kart_denom,
        unlock_time: config.unlock_time,
        distribution_schedule: config.distribution_schedule,
    };

    Ok(resp)
}

pub fn query_state(deps: Deps, block_time: Option<u64>) -> StdResult<StateResponse> {
    let mut state: State = load_state(deps)?;

    if let Some(block_time) = block_time {
        let config = CONFIG.load(deps.storage)?;
        compute_reward(&config, &mut state, block_time);
    }

    Ok(StateResponse {
        total_staker: state.total_staker,
        total_stake_amount: state.total_stake_amount,
        last_distributed: state.last_distributed,
        reward_distributed: state.reward_distributed,
        reward_index: state.reward_index,
    })
}

pub fn query_staker_info(
    deps: Deps,
    staker: String,
    block_time: Option<u64>,
) -> StdResult<StakerInfoResponse> {
    let staker = deps.api.addr_validate(&staker)?;

    let mut staker_info: StakerInfo = user_staking(deps, staker.as_str())?;

    if let Some(block_time) = block_time {
        let config = CONFIG.load(deps.storage)?;
        let mut state: State = load_state(deps)?;

        compute_reward(&config, &mut state, block_time);
        compute_staker_reward(&state, &mut staker_info)?;
    }

    Ok(StakerInfoResponse {
        stake_amount: staker_info.stake_amount,
        pending_reward: staker_info.pending_reward,
        reward_index: staker_info.reward_index,
        reward_claimed: staker_info.reward_claimed,
        unlock: staker_info.unlock,
    })
}
