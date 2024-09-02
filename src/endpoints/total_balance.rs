use crate::utils::{
    dec_format,
    formatting::{format_near_amount, format_tokens},
    rpc::{get_cached_30s, view_account_cached_30s, view_cached_30s},
};

use std::cmp::Reverse;

use itertools::Itertools;
use serde::Deserialize;

use crate::global_state::{get_ft_metadata, get_ft_price, is_spam_token};

#[derive(Debug, Deserialize)]
pub struct WrappedAccountId {
    pub account_id: String,
}

pub async fn get_total_balance(
    input: WrappedAccountId,
) -> Result<impl warp::Reply, warp::Rejection> {
    let WrappedAccountId { account_id } = input;
    let near_balance = view_account_cached_30s(account_id.clone())
        .await
        .map_err(|e| {
            log::warn!("Failed to get NEAR balance of {account_id}: {e:?}");
            warp::reject::reject()
        })?
        .amount;

    let staked_near = get_delegated_validators(&account_id).await;
    let staked_near = match staked_near {
        Ok(staked_near) => {
            let mut staked_near_str = String::new();
            for (pool_id, staked_amount, unstaked_amount, is_unstaked_amount_available) in
                staked_near
                    .into_iter()
                    .filter(|(_, staked, unstaked, _)| *staked != 0 || *unstaked != 0)
                    .sorted_by_key(|(_, staked, unstaked, _)| Reverse(*staked + *unstaked))
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
                            availability = if is_unstaked_amount_available {
                                "Unstaked and ready to claim"
                            } else {
                                "Currently unstaking"
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

    let tokens = get_all_fts_owned(&account_id).await;
    let tokens = {
        let mut tokens_with_price = Vec::new();
        for (token_id, balance) in tokens {
            if is_spam_token(&token_id).await {
                continue;
            }
            if let Some(meta) = get_ft_metadata(&token_id).await {
                let price = get_ft_price(&token_id).await.unwrap_or_default();
                let balance_human_readable = balance as f64 / 10f64.powi(meta.decimals as i32);
                tokens_with_price.push((token_id, balance, balance_human_readable * price));
            }
        }
        tokens_with_price
    };
    let tokens = tokens
        .into_iter()
        .filter(|(_, balance, _)| *balance > 0)
        .sorted_by(|(_, _, balance_1), (_, _, balance_2)| balance_2.partial_cmp(balance_1).unwrap())
        .collect::<Vec<_>>();
    let mut tokens_balance = String::new();
    for (ref token_id, balance, _) in tokens {
        tokens_balance.push_str(&format!(
            "{token_id} {}\n",
            format_tokens(balance, token_id).await,
        ));
    }

    Ok(format!(
        "
NEAR balance: {}

Staked NEAR: {staked_near}

Tokens:
{tokens_balance}
        ",
        format_near_amount(near_balance).await,
    ))
}

async fn get_all_fts_owned(account_id: &str) -> Vec<(String, u128)> {
    #[derive(Debug, Deserialize)]
    struct Response {
        tokens: Vec<Token>,
        #[allow(dead_code)]
        account_id: String,
    }

    #[derive(Debug, Deserialize)]
    struct Token {
        #[allow(dead_code)]
        last_update_block_height: Option<u64>,
        contract_id: String,
        #[serde(with = "dec_format")]
        balance: u128,
    }

    let url = format!("https://api.fastnear.com/v1/account/{account_id}/ft");
    match get_cached_30s::<Response>(&url).await {
        Ok(response) => response
            .tokens
            .into_iter()
            .map(|ft| (ft.contract_id, ft.balance))
            .collect(),
        Err(e) => {
            log::warn!("Failed to get FTs owned by {account_id}: {e:?}");
            Vec::new()
        }
    }
}

async fn get_delegated_validators(
    account_id: &String,
) -> Result<Vec<(String, u128, u128, bool)>, anyhow::Error> {
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

                    // For some reason, unstaked amount always goes +1 yoctonear every time you stake
                    if let Ok(1_000..) = amount_unstaked {
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
                            ) => Ok((
                                pool_id,
                                staked_amount,
                                unstaked_amount,
                                is_unstaked_balance_available,
                            )),
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
