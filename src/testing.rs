use std::collections::HashMap;

use crate::contract::{execute, instantiate, query};
use crate::error::ContractError;
use crate::mock_querier::mock_dependencies;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, StakerInfoResponse, StateResponse, Unlock,
};

use cosmwasm_std::testing::{mock_env, mock_info};
use cosmwasm_std::{from_json, Coin, Decimal, Uint128};

#[test]
fn proper_initialization() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        kart_denom: "kart0000".to_string(),
        unlock_time: 10000,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);

    // we can just call .unwrap() to assert this was a success
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // it worked, let's query the state
    let res = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
    let config: ConfigResponse = from_json(&res).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            owner: "addr0000".to_string(),
            kart_denom: "kart0000".to_string(),
            unlock_time: 10000,
            distribution_schedule: vec![],
        }
    );

    let res = query(
        deps.as_ref(),
        mock_env(),
        QueryMsg::State { block_time: None },
    )
    .unwrap();
    let state: StateResponse = from_json(&res).unwrap();
    assert_eq!(
        state,
        StateResponse {
            total_staker: 0,
            total_stake_amount: Uint128::zero(),
            last_distributed: 0,
            reward_index: HashMap::new(),
            reward_distributed: HashMap::new(),
        }
    );
}

#[test]
fn test_stake_tokens() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        kart_denom: "kart0000".to_string(),
        unlock_time: 10000,
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
                "kart0000".to_string(),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(1000000u128),
                "usk0000".to_string(),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("addr0000", &[Coin::new(100, "kart0000".to_string())]);
    let mut env = mock_env();
    let _res = execute(
        deps.as_mut(),
        env.clone(),
        info.clone(),
        ExecuteMsg::Stake {},
    )
    .unwrap();

    assert_eq!(
        from_json::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            stake_amount: Uint128::from(100u128),
            pending_reward: HashMap::new(),
            reward_index: HashMap::new(),
            reward_claimed: HashMap::new(),
            unlock: None,
        }
    );

    assert_eq!(
        from_json::<StateResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::State { block_time: None }
            )
            .unwrap()
        )
        .unwrap(),
        StateResponse {
            total_staker: 1,
            total_stake_amount: Uint128::from(100u128),
            last_distributed: mock_env().block.time.seconds(),
            reward_index: HashMap::new(),
            reward_distributed: HashMap::new(),
        }
    );

    env.block.time = env.block.time.plus_seconds(150);

    let _res = execute(deps.as_mut(), env, info, ExecuteMsg::Stake {}).unwrap();

    assert_eq!(
        from_json::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            stake_amount: Uint128::from(200u128),
            pending_reward: HashMap::from([
                ("kart0000".to_string(), Uint128::from(1000000u128)),
                ("usk0000".to_string(), Uint128::from(500000u128))
            ]),
            reward_index: HashMap::from([
                (
                    "kart0000".to_string(),
                    Decimal::from_ratio(1000000u128, 100u128)
                ),
                (
                    "usk0000".to_string(),
                    Decimal::from_ratio(500000u128, 100u128)
                )
            ]),
            reward_claimed: HashMap::new(),
            unlock: None,
        }
    );

    assert_eq!(
        from_json::<StateResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::State { block_time: None }
            )
            .unwrap()
        )
        .unwrap(),
        StateResponse {
            total_staker: 1,
            total_stake_amount: Uint128::from(200u128),
            last_distributed: mock_env().block.time.seconds() + 150,
            reward_index: HashMap::from([
                (
                    "kart0000".to_string(),
                    Decimal::from_ratio(1000000u128, 100u128)
                ),
                (
                    "usk0000".to_string(),
                    Decimal::from_ratio(500000u128, 100u128)
                )
            ]),
            reward_distributed: HashMap::new(),
        }
    );
}

#[test]
fn test_unstake() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        kart_denom: "kart0000".to_string(),
        unlock_time: 10000,
        distribution_schedule: vec![],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    let info = mock_info("addr0000", &[Coin::new(100, "kart0000".to_string())]);
    let _res = execute(deps.as_mut(), mock_env(), info, ExecuteMsg::Stake {}).unwrap();

    // unbond 150 tokens; failed
    let msg = ExecuteMsg::Unstake {
        amount: Uint128::from(150u128),
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap_err();
    match res {
        ContractError::InsufficientToken {} => {}
        _ => panic!("Must return generic error"),
    };

    // normal unstake
    let msg = ExecuteMsg::Unstake {
        amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);

    let env = mock_env();
    let _res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    assert_eq!(
        from_json::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            stake_amount: Uint128::zero(),
            pending_reward: HashMap::new(),
            reward_index: HashMap::new(),
            reward_claimed: HashMap::new(),
            unlock: Some(vec![Unlock {
                amount: Uint128::from(100u128),
                at: env.block.time.seconds() + 10000
            }]),
        }
    );
}

#[test]
fn test_withdraw() {
    let mut deps = mock_dependencies(&[]);

    let msg = InstantiateMsg {
        kart_denom: "kart0000".to_string(),
        unlock_time: 10000,
        distribution_schedule: vec![
            (
                mock_env().block.time.seconds(),
                mock_env().block.time.seconds() + 100,
                Uint128::from(1000000u128),
                "kart0000".to_string(),
            ),
            (
                mock_env().block.time.seconds() + 100,
                mock_env().block.time.seconds() + 200,
                Uint128::from(1000000u128),
                "usk0000".to_string(),
            ),
        ],
    };

    let info = mock_info("addr0000", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

    // stake 100 tokens

    let info = mock_info("addr0000", &[Coin::new(100, "kart0000".to_string())]);
    let mut env = mock_env();
    let _res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Stake {}).unwrap();

    // 100 seconds passed
    // 1,000,000 kart0000 rewards distributed
    env.block.time = env.block.time.plus_seconds(150);

    // normal unstake
    let msg = ExecuteMsg::Unstake {
        amount: Uint128::from(100u128),
    };

    let info = mock_info("addr0000", &[]);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_json::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            stake_amount: Uint128::zero(),
            pending_reward: HashMap::from([
                ("kart0000".to_string(), Uint128::from(1000000u128)),
                ("usk0000".to_string(), Uint128::from(500000u128))
            ]),
            reward_index: HashMap::from([
                (
                    "kart0000".to_string(),
                    Decimal::from_ratio(1000000u128, 100u128)
                ),
                (
                    "usk0000".to_string(),
                    Decimal::from_ratio(500000u128, 100u128)
                )
            ]),
            reward_claimed: HashMap::new(),
            unlock: Some(vec![Unlock {
                amount: Uint128::from(100u128),
                at: env.clone().block.time.seconds() + 10000
            }]),
        }
    );

    let msg = ExecuteMsg::Withdraw { id: 0 };
    let info = mock_info("addr0000", &[]);
    env.block.time = env.block.time.plus_seconds(10000);
    let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

    assert_eq!(
        from_json::<StakerInfoResponse>(
            &query(
                deps.as_ref(),
                mock_env(),
                QueryMsg::StakerInfo {
                    staker: "addr0000".to_string(),
                    block_time: None,
                },
            )
            .unwrap(),
        )
        .unwrap(),
        StakerInfoResponse {
            stake_amount: Uint128::zero(),
            pending_reward: HashMap::new(),
            reward_index: HashMap::new(),
            reward_claimed: HashMap::new(),
            unlock: None,
        }
    );
}
