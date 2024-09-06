use crate::utils::{
    formatting::format_near_amount,
    rpc::{get_cached_30s, view_account_cached_30s, view_cached_30s},
};

use std::cmp::Reverse;

use itertools::Itertools;
use serde::Deserialize;

// For some reason, unstaked amount always goes +1 yoctonear every time you stake
pub const NOT_STAKING_THRESHOLD: u128 = 1_000;

#[derive(Debug, Deserialize)]
pub struct GetStakingInput {
    pub account_id: String,
}

pub async fn get_staking(input: GetStakingInput) -> Result<impl warp::Reply, warp::Rejection> {
    let GetStakingInput { account_id } = input;
    let near_balance = view_account_cached_30s(account_id.clone())
        .await
        .map_err(|e| {
            log::warn!("Failed to get NEAR balance of {account_id}: {e:?}");
            warp::reject::reject()
        })?
        .amount;

    let staked_near = format_staking_info(&account_id).await;

    Ok(format!(
        "
NEAR balance: {}

Staked NEAR: {staked_near}
        ",
        format_near_amount(near_balance).await,
    ))
}

pub struct StakingData {
    pub pool_id: String,
    pub staked_amount: u128,
    pub unstaked_amount: u128,
    pub is_unstaked_balance_available: bool,
}

pub async fn get_delegated_validators(
    account_id: &String,
) -> Result<Vec<StakingData>, anyhow::Error> {
    #[derive(Debug, Deserialize)]
    struct Response {
        pools: Vec<Pool>,
        #[allow(dead_code)]
        account_id: String,
    }

    #[derive(Debug, Deserialize)]
    struct Pool {
        pool_id: String,
        #[allow(dead_code)]
        last_update_block_height: Option<u64>,
    }

    let url = format!("https://api.fastnear.com/v1/account/{account_id}/staking");
    match get_cached_30s::<Response>(&url).await {
        Ok(response) => {
            let pools = response.pools.into_iter().map(|pool| pool.pool_id);
            let mut amounts = Vec::new();
            for pool_id in pools {
                amounts.push(async {
                    let amount_staked = view_cached_30s::<_, String>(
                        &pool_id,
                        "get_account_staked_balance",
                        serde_json::json!({"account_id": account_id}),
                    )
                    .await
                    .map(|balance| balance.parse::<u128>().unwrap_or_default());
                    let amount_unstaked = view_cached_30s::<_, String>(
                        &pool_id,
                        "get_account_unstaked_balance",
                        serde_json::json!({"account_id": account_id}),
                    )
                    .await
                    .map(|balance| balance.parse::<u128>().unwrap_or_default());

                    if let Ok(NOT_STAKING_THRESHOLD..) = amount_unstaked {
                        let is_unstaked_balance_available = view_cached_30s::<_, bool>(
                            &pool_id,
                            "is_account_unstaked_balance_available",
                            serde_json::json!({"account_id": account_id}),
                        )
                        .await;
                        (
                            pool_id,
                            amount_staked,
                            amount_unstaked,
                            is_unstaked_balance_available,
                        )
                    } else {
                        (pool_id, amount_staked, Ok(0), Ok(false))
                    }
                });
            }
            futures_util::future::join_all(amounts)
                .await
                .into_iter()
                .map(
                    |(pool_id, staked_amount, unstaked_amount, is_unstaked_balance_available)| {
                        match (
                            staked_amount,
                            unstaked_amount,
                            is_unstaked_balance_available,
                        ) {
                            (
                                Ok(staked_amount),
                                Ok(unstaked_amount),
                                Ok(is_unstaked_balance_available),
                            ) => Ok(StakingData {
                                pool_id,
                                staked_amount,
                                unstaked_amount,
                                is_unstaked_balance_available,
                            }),
                            (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => Err(e),
                        }
                    },
                )
                .collect()
        }
        Err(e) => {
            log::warn!("Failed to get validators delegated by {account_id}: {e:?}");
            Err(e)
        }
    }
}

pub async fn format_staking_info(account_id: &String) -> String {
    let staked_near = get_delegated_validators(account_id).await;
    let staked_near = match staked_near {
        Ok(staked_near) => {
            let mut staked_near_str = String::new();
            for StakingData {
                pool_id,
                staked_amount,
                unstaked_amount,
                is_unstaked_balance_available,
            } in staked_near
                .into_iter()
                .filter(|d| d.staked_amount != 0 || d.unstaked_amount != 0)
                .sorted_by_key(|d| Reverse(d.staked_amount + d.unstaked_amount))
            {
                staked_near_str.push_str(&format!(
                    "\n- {pool_id} : *{staked_amount}*{unstaked}",
                    staked_amount = format_near_amount(staked_amount).await,
                    // For some reason, unstaked amount always goes +1 yoctonear every time you stake
                    unstaked = if unstaked_amount <= 1_000 {
                        "".to_string()
                    } else {
                        format!(
                            ". {availability} *{unstaked}*",
                            availability = if is_unstaked_balance_available {
                                "Unstaked and ready to claim"
                            } else {
                                "Currently in the process of unstaking, will be available in 2-3 days"
                            },
                            unstaked = format_near_amount(unstaked_amount).await,
                        )
                    }
                ));
            }
            staked_near_str
        }
        Err(e) => {
            log::warn!("Failed to get staked NEAR of {account_id}: {e:?}");
            "Failed to get information, please try again later or report in @intearchat".to_string()
        }
    };
    if staked_near.is_empty() {
        "No staked NEAR".to_string()
    } else {
        staked_near
    }
}
