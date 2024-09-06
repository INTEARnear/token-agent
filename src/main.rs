mod endpoints;
mod global_state;
mod utils;

use std::sync::Arc;

use endpoints::{
    stake::{create_stake_transaction, StakeInput},
    staking::{get_staking, GetStakingInput},
    token_prices::{get_token_prices, TokenPricesInput},
    total_balance::{get_total_balance, WrappedAccountId},
    unstake::{create_unstake_transaction, UnstakeInput},
};
use global_state::Tokens;
use tokio::sync::RwLock;
use utils::rpc::get_reqwest_client;
use warp::{filters::header::header, reply::Response, Filter};

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
            let result: Result<(), Box<dyn std::error::Error>> = async {
                interval.tick().await;
                let new_tokens = Tokens {
                    tokens: get_reqwest_client()
                        .get("https://prices.intear.tech/tokens")
                        .send()
                        .await?
                        .json()
                        .await?,
                    spam_tokens: get_reqwest_client()
                        .get("https://prices.intear.tech/token-spam-list")
                        .send()
                        .await?
                        .json()
                        .await?,
                };
                log::info!("Cache refreshed");
                *global_state::TOKENS.get().unwrap().write().await = new_tokens;
                Ok(())
            }
            .await;
            if let Err(err) = result {
                log::error!("Failed to refresh cache: {err:?}");
            }
        }
    });

    let manifest = warp::path!(".well-known" / "ai-plugin.json")
        .and(header("Host"))
        .map(|host: String| {
            log::info!("Sending ai-plugin.json");

            let mut res = Response::new(
                match host.split('.').next() {
                    Some("staking-agent") => {
                        include_str!("../staking-agent.json")
                    }
                    Some("tokens-agent") => {
                        include_str!("../tokens-agent.json")
                    }
                    _ => {
                        #[cfg(feature = "local-debug-agent")]
                        {
                            include_str!(concat!("../", env!("DEBUG_AGENT"), "-agent.json"))
                        }
                        #[cfg(not(feature = "local-debug-agent"))]
                        {
                            log::warn!("Unknown host: {host}");
                            let mut response = Response::new("Unknown host".into());
                            *response.status_mut() = warp::http::StatusCode::BAD_REQUEST;
                            return response;
                        }
                    }
                }
                .into(),
            );
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
    let staking = warp::path("staking")
        .and(warp::query::query::<GetStakingInput>())
        .and_then(|input| {
            log::info!("Sending staking for tokens: {input:?}");
            get_staking(input)
        });
    let stake = warp::path("stake")
        .and(warp::query::query::<StakeInput>())
        .and_then(|input| {
            log::info!("Creating stake transaction for {input:?}");
            create_stake_transaction(input)
        });
    let unstake = warp::path("unstake")
        .and(warp::query::query::<UnstakeInput>())
        .and_then(|input| {
            log::info!("Creating unstake transaction for {input:?}");
            create_unstake_transaction(input)
        });
    let api = total_balance
        .or(token_prices)
        .or(staking)
        .or(stake)
        .or(unstake);

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
