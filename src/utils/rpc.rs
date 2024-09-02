use crate::utils::dec_format;
use base64::Engine;
use cached::proc_macro::cached;
use lazy_static::lazy_static;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

pub const RPC_URLS: &[&str] = &[
    "https://rpc.shitzuapes.xyz",
    "https://rpc.mainnet.near.org",
    "https://near.lava.build",
];

macro_rules! try_rpc {
    (|$rpc_url: ident| $body: block) => {{
        let mut i = 0;
        loop {
            let result: Result<_, _> = async {
                let $rpc_url = RPC_URLS[i];
                let res = $body;
                res
            }
            .await;
            match result {
                Ok(res) => break Ok(res),
                Err(err) => {
                    if i >= RPC_URLS.len() - 1 {
                        break Err(err);
                    }
                    i += 1;
                }
            }
        }
    }};
}

#[derive(Deserialize, Debug)]
pub struct RpcResponse<T> {
    #[allow(dead_code)]
    id: Option<String>,
    #[allow(dead_code)]
    jsonrpc: String,
    result: T,
}

lazy_static! {
    static ref CLIENT: reqwest::Client = reqwest::Client::builder()
        .user_agent("Intear Xeon")
        .build()
        .expect("Failed to create reqwest client");
}

pub fn get_reqwest_client() -> &'static reqwest::Client {
    &CLIENT
}

pub async fn rpc<I: Serialize, O: DeserializeOwned>(
    data: I,
) -> Result<RpcResponse<O>, anyhow::Error> {
    try_rpc!(|rpc_url| {
        Ok(get_reqwest_client()
            .post(rpc_url)
            .json(&data)
            .send()
            .await?
            .json::<RpcResponse<O>>()
            .await?)
    })
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AccountInfo {
    #[serde(with = "dec_format")]
    pub amount: u128,
    #[serde(with = "dec_format")]
    pub locked: u128,
    pub code_hash: String,
    pub storage_usage: u64,
    pub storage_paid_at: u64,
    pub block_height: u64,
    pub block_hash: String,
}

pub async fn view_account_not_cached(account_id: &str) -> Result<AccountInfo, anyhow::Error> {
    let response = rpc::<_, AccountInfo>(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": "query",
        "params": {
            "request_type": "view_account",
            "finality": "final",
            "account_id": account_id,
        }
    }))
    .await?
    .result;
    Ok(response)
}

#[cached(time = 30, result = true, size = 1000)]
pub async fn view_account_cached_30s(account_id: String) -> Result<AccountInfo, anyhow::Error> {
    view_account_not_cached(&account_id).await
}

async fn _get_internal(uri: &str) -> Result<serde_json::Value, anyhow::Error> {
    Ok(get_reqwest_client().get(uri).send().await?.json().await?)
}

#[cached(time = 30, result = true, size = 50)]
async fn _get_cached_30s(uri: String) -> Result<serde_json::Value, anyhow::Error> {
    _get_internal(&uri).await
}

pub async fn get_cached_30s<O: DeserializeOwned>(uri: &str) -> Result<O, anyhow::Error> {
    let res = _get_cached_30s(uri.to_string()).await?;
    Ok(serde_json::from_value(res)?)
}

#[derive(Deserialize, Debug)]
struct RpcResponseCallFunctionView {
    #[serde(deserialize_with = "from_bytes")]
    result: String,
    #[allow(dead_code)]
    logs: Vec<String>,
    #[allow(dead_code)]
    block_height: u64,
    #[allow(dead_code)]
    block_hash: String,
}

fn from_bytes<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes = Vec::<u8>::deserialize(deserializer)?;
    String::from_utf8(bytes).map_err(|_| serde::de::Error::custom("Invalid UTF-8 result array"))
}

async fn _internal_view(
    contract_id: &str,
    method_name: &str,
    args: &str,
) -> Result<serde_json::Value, anyhow::Error> {
    let response = rpc::<_, RpcResponseCallFunctionView>(serde_json::json!({
        "jsonrpc": "2.0",
        "id": "dontcare",
        "method": "query",
        "params": {
            "request_type": "call_function",
            "finality": "final",
            "account_id": contract_id,
            "method_name": method_name,
            "args_base64": base64::prelude::BASE64_STANDARD.encode(args.as_bytes()),
        }
    }))
    .await?
    .result;
    Ok(serde_json::from_str(&response.result)?)
}

#[cached(time = 30, result = true, size = 1000)]
async fn _internal_view_cached_30s(
    contract_id: String,
    method_name: String,
    args: String,
) -> Result<serde_json::Value, anyhow::Error> {
    _internal_view(&contract_id, &method_name, &args).await
}

pub async fn view_cached_30s<I: Serialize, O: DeserializeOwned>(
    contract_id: impl AsRef<str>,
    method_name: impl AsRef<str>,
    args: I,
) -> Result<O, anyhow::Error> {
    let contract_id = contract_id.as_ref().to_string();
    let method_name = method_name.as_ref().to_string();
    let args = serde_json::to_string(&args)?;
    let res = _internal_view_cached_30s(contract_id, method_name, args).await;
    Ok(serde_json::from_value(res?)?)
}
