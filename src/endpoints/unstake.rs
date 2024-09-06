use bigdecimal::{BigDecimal, FromPrimitive, Zero};
use serde::Deserialize;

use super::staking::{get_delegated_validators, NOT_STAKING_THRESHOLD};

#[derive(Debug, Deserialize)]
pub struct UnstakeInput {
    pub account_id: String,
    pub pool_account_id: Option<String>,
    pub amount: Option<BigDecimal>,
}

pub async fn create_unstake_transaction(
    input: UnstakeInput,
) -> Result<impl warp::Reply, warp::Rejection> {
    let UnstakeInput {
        account_id,
        pool_account_id,
        amount,
    } = input;
    let staking_data = get_delegated_validators(&account_id).await.map_err(|e| {
        log::warn!("Failed to get staking data of {account_id}: {e:?}");
        warp::reject::reject()
    })?;
    if let Some(pool_account_id) = pool_account_id {
        let pool_data = staking_data.iter().find(|d| d.pool_id == pool_account_id);
        let Some(pool_data) = pool_data else {
            return Ok("You are not staking in this pool.".to_string());
        };
        if let Some(amount) = amount {
            let yocto = amount * BigDecimal::from_u128(10u128.pow(24)).unwrap();
            if BigDecimal::from_u128(pool_data.unstaked_amount).unwrap() >= yocto {
                return Ok(format!(
                    r#"
[
    {{
        "methodName": "withdraw",
        "args": "{{\"amount\": \"{yocto}\"}}",
        "gas": "50000000000000",
        "deposit": "0",
        "contractName": "{pool_account_id}"
    }}
]

Use this data to call `generate-transaction` tool to generate a transaction.
    "#
                ));
            }
            if BigDecimal::from_u128(pool_data.staked_amount).unwrap() < yocto {
                return Ok(format!(
                    "You are only staking {} NEAR in this pool. {} NEAR is available for withdrawal.",
                    BigDecimal::from_u128(pool_data.staked_amount).unwrap()
                        / BigDecimal::from_u128(10u128.pow(24)).unwrap(),
                    BigDecimal::from_u128(pool_data.unstaked_amount).unwrap()
                        / BigDecimal::from_u128(10u128.pow(24)).unwrap()
                ));
            }
            Ok(format!(
                r#"
    let yocto = amount * BigDecimal::from_u128(10u128.pow(24)).unwrap();
    Ok(format!(r#"
[
    {{
      "methodName": "unstake",
      "args": "{{\"amount\": \"{yocto}\"}}",
      "gas": "50000000000000",
      "deposit": "0",
      "contractName": "{pool_account_id}"
    }}
]

Use this data to call `generate-transaction` tool to generate a transaction. After that, let the user know that unstaking takes 2-3 days on average. Don't forget to use the correct syntax for the `generate-transaction` tool, the provided JSON is only a part of the arguments.
    "#
            ))
        } else {
            if pool_data.unstaked_amount != 0 {
                return Ok(format!(
                    r#"
[
    {{
        "methodName": "withdraw_all",
        "args": "{{}}",
        "gas": "50000000000000",
        "deposit": "0",
        "contractName": "{pool_account_id}"
    }}
]

Use this data to call `generate-transaction` tool to generate a transaction. After that, let the user know that {} NEAR has been withdrawn from the pool{} Don't forget to use the correct syntax for the `generate-transaction` tool, the provided JSON is only a part of the arguments.
    "#,
                    BigDecimal::from_u128(pool_data.unstaked_amount).unwrap()
                        / BigDecimal::from_u128(10u128.pow(24)).unwrap(),
                    if pool_data.staked_amount <= NOT_STAKING_THRESHOLD {
                        ".".to_string()
                    } else {
                        format!(
                            ", and {} NEAR is available for unstake. To unstake, repeat the same tool",
                            BigDecimal::from_u128(pool_data.staked_amount).unwrap()
                                / BigDecimal::from_u128(10u128.pow(24)).unwrap()
                        )
                    }
                ));
            }
            if pool_data.staked_amount <= NOT_STAKING_THRESHOLD {
                // TODO try withdraw instead of unstake
                return Ok("You are not staking in this pool.".to_string());
            }
            Ok(format!(
                r#"
    let yocto = amount * BigDecimal::from_u128(10u128.pow(24)).unwrap();
    Ok(format!(r#"
[
    {{
      "methodName": "unstake_all",
      "args": "{{}}",
      "gas": "50000000000000",
      "deposit": "0",
      "contractName": "{pool_account_id}"
    }}
]

Use this data to call `generate-transaction` tool to generate a transaction. After that, let the user know that unstaking takes 2-3 days on average. Don't forget to use the correct syntax for the `generate-transaction` tool, the provided JSON is only a part of the arguments.
    "#
            ))
        }
    } else {
        let mut data = Vec::new();
        let mut max_unstake_amount = amount.clone();

        // Withdraw unstaked
        for pool_data in staking_data.iter() {
            let to_withdraw = if let Some(max_unstake_amount) = max_unstake_amount.as_mut() {
                if *max_unstake_amount == BigDecimal::zero() {
                    break;
                }
                let to_withdraw = max_unstake_amount
                    .clone()
                    .min(BigDecimal::from_u128(pool_data.unstaked_amount).unwrap());
                *max_unstake_amount -= to_withdraw.clone();
                to_withdraw
            } else {
                BigDecimal::from_u128(pool_data.unstaked_amount).unwrap()
            };
            data.push(format!(
                r#"
    {{
        "methodName": "withdraw",
        "args": "{{\"amount\": \"{to_withdraw}\"}}",
        "gas": "50000000000000",
        "deposit": "0",
        "contractName": "{}"
    }}"#,
                pool_data.pool_id
            ));
        }

        // Unstake
        for pool_data in staking_data.iter() {
            if pool_data.staked_amount <= NOT_STAKING_THRESHOLD {
                continue;
            }
            let to_unstake = if let Some(max_unstake_amount) = max_unstake_amount.as_mut() {
                if *max_unstake_amount == BigDecimal::zero() {
                    break;
                }
                let to_unstake = max_unstake_amount
                    .clone()
                    .min(BigDecimal::from_u128(pool_data.staked_amount).unwrap());
                *max_unstake_amount -= to_unstake.clone();
                to_unstake
            } else {
                BigDecimal::from_u128(pool_data.staked_amount).unwrap()
            };
            data.push(format!(
                r#"
    {{
        "methodName": "unstake",
        "args": "{{\"amount\": \"{to_unstake}\"}}",
        "gas": "50000000000000",
        "deposit": "0",
        "contractName": "{}"
    }}"#,
                pool_data.pool_id
            ));
        }
        if data.is_empty() {
            return Ok("You are not staking in any pool.".to_string());
        }
        Ok(format!(
            r#"Use `generate-transaction` with {{"transactions":[{}]}}"#,
            data.join(",")
        ))
    }
}
