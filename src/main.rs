mod endpoints;
mod global_state;
mod utils;

use std::sync::Arc;

use endpoints::{
    token_prices::{get_token_prices, TokenPricesInput},
    total_balance::{get_total_balance, WrappedAccountId},
};
use global_state::Tokens;
use tokio::sync::RwLock;
use utils::rpc::get_reqwest_client;
use warp::{reply::Response, Filter};

#[tokio::main]
async fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();

    global_state::TOKENS
        .get_or_init(|| async {
            Arc::new(RwLock::new(Tokens {
                tokens: get_reqwest_client()
                    .get("https://prices.intear.tech/tokens")
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap(),
                spam_tokens: get_reqwest_client()
                    .get("https://prices.intear.tech/token-spam-list")
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap(),
            }))
        })
        .await;
    tokio::spawn(async {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            let new_tokens = Tokens {
                tokens: get_reqwest_client()
                    .get("https://prices.intear.tech/tokens")
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap(),
                spam_tokens: get_reqwest_client()
                    .get("https://prices.intear.tech/token-spam-list")
                    .send()
                    .await
                    .unwrap()
                    .json()
                    .await
                    .unwrap(),
            };
            log::info!("Cache cleared");
            *global_state::TOKENS.get().unwrap().write().await = new_tokens;
        }
    });

    let manifest = warp::path!(".well-known" / "ai-plugin.json").map(|| {
        log::info!("Sending ai-plugin.json");
        let mut res = Response::new(include_str!("../ai-plugin.json").into());
        res.headers_mut()
            .insert("content-type", "application/json".parse().unwrap());
        res
    });

    let total_balance = warp::path("total-balance")
        .and(warp::query::query::<WrappedAccountId>())
        .and_then(|input| {
            log::info!("Sending total-balance for account_id: {input:?}");
            get_total_balance(input)
        });
    let token_prices = warp::path("token-prices")
        .and(warp::query::query::<TokenPricesInput>())
        .and_then(|input| {
            log::info!("Sending token-prices for tokens: {input:?}");
            get_token_prices(input)
        });
    let api = total_balance.or(token_prices);

    let routes = manifest
        .or(api)
        .or(warp::any().and(warp::path::full()).map(|path| {
            log::debug!("{path:?}");
            let mut res = Response::new("Not Found".into());
            *res.status_mut() = warp::http::StatusCode::NOT_FOUND;
            res
        }));

    log::info!("Server started");
    warp::serve(routes)
        .run(if cfg!(debug_assertions) {
            ([127, 0, 0, 1], 3030)
        } else {
            ([0, 0, 0, 0], 80)
        })
        .await;
}
