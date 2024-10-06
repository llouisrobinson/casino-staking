use std::collections::HashMap;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Decimal, Uint128};

use crate::state::Config;

#[cw_serde]
pub struct InstantiateMsg {
    pub kart_denom: String,
    pub unlock_time: u64,
    pub distribution_schedule: Vec<(u64, u64, Uint128, String)>,
}

#[cw_serde]
pub enum ExecuteMsg {
    Stake {},
    Unstake {
        amount: Uint128,
    },
    Withdraw {
        id: usize,
    },
    // claim pending rewards
    Claim {},
    Unlock {
        amount: Uint128,
        denom: String
    },
    SetDistribution {
        reward_denom: String,
        start_date: u64,
        end_date: u64,
        amount: Uint128,
    },
    UpdateConfig {
        config: Config,
    },
}

// query msgs

#[cw_serde]
pub enum QueryMsg {
    Config {},
    State {
        block_time: Option<u64>,
    },
    StakerInfo {
        staker: String,
        block_time: Option<u64>,
    },
}

// We define a custom struct for each query response
#[cw_serde]
pub struct ConfigResponse {
    pub owner: String,

    pub kart_denom: String,

    pub unlock_time: u64,

    pub distribution_schedule: Vec<(u64, u64, Uint128, String)>,
}

#[cw_serde]
pub struct StateResponse {
    pub total_staker: u64,
    pub total_stake_amount: Uint128,
    pub last_distributed: u64,
    pub reward_index: HashMap<String, Decimal>,
    pub reward_distributed: HashMap<String, Uint128>,
}

#[cw_serde]
pub struct Unlock {
    pub amount: Uint128,
    pub at: u64,
}

#[cw_serde]
pub struct StakerInfoResponse {
    pub stake_amount: Uint128,
    pub pending_reward: HashMap<String, Uint128>,
    pub reward_index: HashMap<String, Decimal>,
    pub reward_claimed: HashMap<String, Uint128>,
    pub unlock: Option<Vec<Unlock>>,
}
