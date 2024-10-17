use crate::utils::{
    formatting::{format_near_amount, format_tokens},
    rpc::{get_cached_30s, view_account_cached_30s},
};

use itertools::Itertools;
use near_primitives::types::{AccountId, BlockHeight};
use serde::Deserialize;

use crate::global_state::{get_ft_metadata, get_ft_price, is_spam_token};

use super::staking::format_staking_info;

#[derive(Debug, Deserialize)]
pub struct WrappedAccountId {
    pub account_id: AccountId,
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

    let staked_near = format_staking_info(&account_id).await;

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

async fn get_all_fts_owned(account_id: &AccountId) -> Vec<(AccountId, u128)> {
    #[derive(Debug, Deserialize)]
    struct Response {
        tokens: Vec<Token>,
        #[allow(dead_code)]
        account_id: AccountId,
    }

    #[derive(Debug, Deserialize)]
    struct Token {
        #[allow(dead_code)]
        last_update_block_height: Option<BlockHeight>,
        contract_id: AccountId,
        // can be an empty string
        balance: String,
    }

    let url = format!("https://api.fastnear.com/v1/account/{account_id}/ft");
    match get_cached_30s::<Response>(&url).await {
        Ok(response) => response
            .tokens
            .into_iter()
            .map(|ft| (ft.contract_id, ft.balance.parse().unwrap_or_default()))
            .collect(),
        Err(e) => {
            log::warn!("Failed to get FTs owned by {account_id}: {e:?}");
            Vec::new()
        }
    }
}
