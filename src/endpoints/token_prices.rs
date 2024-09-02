use serde::Deserialize;

use crate::{
    global_state::Token,
    utils::{formatting::format_usd_amount, rpc::get_cached_30s},
};

#[derive(Debug, Deserialize)]
pub struct TokenPricesInput {
    #[serde(deserialize_with = "from_comma_separated")]
    tokens: Vec<String>,
}

fn from_comma_separated<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = serde::Deserialize::deserialize(deserializer)?;
    Ok(s.split(',').map(|s| s.to_string()).collect())
}

pub async fn get_token_prices(
    input: TokenPricesInput,
) -> Result<impl warp::Reply, warp::Rejection> {
    let TokenPricesInput { tokens } = input;
    let mut search_results = Vec::new();
    for token in tokens {
        search_results.push(async {
            let token = token;
            let results = search_tokens(&token).await;
            let exact_matches = results
                .iter()
                .filter(|result| {
                    result.metadata.symbol.to_lowercase().trim_matches('$')
                        == token.to_lowercase().trim_matches('$')
                        || result.metadata.name.to_lowercase().trim_matches('$')
                            == token.to_lowercase().trim_matches('$')
                })
                .collect::<Vec<_>>();
            if exact_matches.is_empty() {
                results
            } else {
                exact_matches.into_iter().cloned().collect()
            }
        });
    }
    let results = futures_util::future::join_all(search_results)
        .await
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let mut response = Vec::new();
    for result in results {
        response.push(format!(
            "Ticker `{}`, Name `{}`, Contract `{}`, Price {}",
            result.metadata.symbol,
            result.metadata.name,
            result.account_id,
            format_usd_amount(result.price_usd.to_string().parse().unwrap_or_default())
        ));
    }
    Ok(warp::reply::json(&response))
}

async fn search_tokens(query: &str) -> Vec<Token> {
    get_cached_30s(&format!(
        "https://prices.intear.tech/token-search?q={query}"
    ))
    .await
    .unwrap_or_default()
}
