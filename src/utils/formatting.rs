use near_primitives::types::AccountId;

use crate::global_state::{get_ft_metadata, get_ft_price};

pub const NEAR_DECIMALS: u32 = 24;
pub const WRAP_NEAR: &str = "wrap.near";

pub async fn format_near_amount(amount: u128) -> String {
    if amount == 0 {
        "0 NEAR".to_string()
    } else if amount < 10u128.pow(18) {
        format!("{amount} yoctoNEAR")
    } else {
        format!(
            "{}{}",
            format_token_amount(amount, NEAR_DECIMALS, "NEAR"),
            if amount != 0 {
                format!(
                    " (${:.02})",
                    (amount as f64 / 10u128.pow(NEAR_DECIMALS) as f64)
                        * get_ft_price(&WRAP_NEAR.parse().unwrap())
                            .await
                            .unwrap_or_default()
                )
            } else {
                "".to_string()
            }
        )
    }
}

pub async fn format_tokens(amount: u128, token: &AccountId) -> String {
    if let Some(metadata) = get_ft_metadata(token).await {
        format!(
            "{}{}",
            format_token_amount(amount, metadata.decimals, &metadata.symbol),
            if amount != 0 {
                if let Some(price) = get_ft_price(token).await {
                    if price != 0f64 {
                        format!(
                            " (${:.02})",
                            (amount as f64 / 10u128.pow(metadata.decimals) as f64) * price
                        )
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            }
        )
    } else {
        format!("{amount} <unknown token>")
    }
}

pub fn format_token_amount(amount: u128, decimals: u32, symbol: &str) -> String {
    if decimals == 0 {
        return format!("{amount} {symbol}");
    }
    if amount == 0 {
        return format!("0 {symbol}");
    }
    let precision = 12.min(decimals);
    let token_float: f64 = (amount / 10u128.pow(decimals - precision)) as f64
        / (10u128.pow(decimals) / 10u128.pow(decimals - precision)) as f64;
    let s = if token_float >= 1_000_000.0 {
        format!("{token_float:.0}")
    } else if token_float >= 10.0 {
        format!("{token_float:.2}")
    } else if token_float >= 1.0 {
        format!("{token_float:.3}")
    } else if token_float >= 1.0 / 1e12 {
        let digits = -token_float.abs().log10().floor() as usize + 2;
        format!("{token_float:.*}", digits)
    } else {
        "0".to_string()
    };
    format!(
        "{amount} {symbol}",
        amount = if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.')
        } else {
            &s
        }
    )
}

pub fn format_usd_amount(amount: f64) -> String {
    format!(
        "${amount:.0$}",
        (2 - amount.log10() as isize).max(0) as usize
    )
}
