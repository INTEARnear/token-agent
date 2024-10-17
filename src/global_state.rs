use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bigdecimal::BigDecimal;
use lazy_static::lazy_static;
use near_primitives::serialize::dec_format;
use near_primitives::types::AccountId;
use serde::{Deserialize, Serialize};
use tokio::sync::{OnceCell, RwLock};

lazy_static! {
    pub static ref TOKENS: OnceCell<Arc<RwLock<Tokens>>> = OnceCell::new();
}

pub async fn is_spam_token(token: &AccountId) -> bool {
    TOKENS
        .get()
        .unwrap()
        .read()
        .await
        .spam_tokens
        .contains(token)
}

pub async fn get_ft_metadata(token: &AccountId) -> Option<TokenMetadataWithoutIcon> {
    TOKENS
        .get()
        .unwrap()
        .read()
        .await
        .tokens
        .get(token)
        .map(|t| t.metadata.clone())
}

pub async fn get_ft_price(token: &AccountId) -> Option<f64> {
    TOKENS
        .get()
        .unwrap()
        .read()
        .await
        .tokens
        .get(token)
        .map(|t| t.price_usd_hardcoded.to_string().parse().unwrap())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Tokens {
    pub tokens: HashMap<AccountId, Token>,
    pub spam_tokens: HashSet<AccountId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub account_id: AccountId,
    #[serde(with = "serde_bigdecimal")]
    pub price_usd_raw: BigDecimal,
    #[serde(with = "serde_bigdecimal")]
    pub price_usd: BigDecimal,
    #[serde(with = "serde_bigdecimal")]
    pub price_usd_hardcoded: BigDecimal,
    pub metadata: TokenMetadataWithoutIcon,
    #[serde(with = "dec_format")]
    pub total_supply: u128,
    #[serde(with = "dec_format")]
    pub circulating_supply: u128,
    #[serde(with = "dec_format")]
    pub circulating_supply_excluding_team: u128,
    pub reputation: TokenScore,
    pub socials: HashMap<String, String>,
    pub slug: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenMetadataWithoutIcon {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Default, Copy)]
pub enum TokenScore {
    Spam,
    #[default]
    Unknown,
    NotFake,
    Reputable,
}

mod serde_bigdecimal {
    use bigdecimal::BigDecimal;
    use serde::{de::Error, Deserialize, Deserializer, Serializer};
    use std::str::FromStr;

    pub fn serialize<S>(value: &BigDecimal, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BigDecimal, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        BigDecimal::from_str(&s).map_err(D::Error::custom)
    }
}
