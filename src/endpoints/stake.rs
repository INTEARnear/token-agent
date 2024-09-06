use bigdecimal::{BigDecimal, FromPrimitive};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StakeInput {
    pub pool_account_id: String,
    pub amount: BigDecimal,
}

pub async fn create_stake_transaction(
    input: StakeInput,
) -> Result<impl warp::Reply, warp::Rejection> {
    let StakeInput {
        pool_account_id,
        amount,
    } = input;
    let yocto = amount * BigDecimal::from_u128(10u128.pow(24)).unwrap();
    Ok(format!(
        r#"
[
    {{
      "methodName": "deposit_and_stake",
      "args": "{{}}",
      "gas": "50000000000000",
      "deposit": "{yocto}",
      "contractName": "{pool_account_id}"
    }}
]

Use this data to call `generate-transaction` tool to generate a transaction.
    "#
    ))
}
