use bigdecimal::{BigDecimal, FromPrimitive};
use num_traits::ToPrimitive;
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
    let yocto = amount.clone() * BigDecimal::from_u128(10u128.pow(24)).unwrap();
    let yocto = ToPrimitive::to_u128(&yocto);
    let Some(yocto) = yocto else {
        log::warn!("Failed to convert amount to yoctoNEAR: {amount}");
        return Ok(r#"Say that the agent failed to convert the amount to yoctoNEAR, this is a bug, and to ask in https://t.me/intearchat for support."#.to_string());
    };
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
