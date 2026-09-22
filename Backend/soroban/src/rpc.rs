use std::{
    collections::{HashMap, VecDeque},
    future::Future,
    sync::Mutex,
    time::Duration,
};

use crate::xdr::{AccountEntry, Hash, Limits, TransactionEnvelope, WriteXdr};
use async_trait::async_trait;
use jsonrpsee_core::{client::ClientT, params::ObjectParams};
use serde::{Serialize, de::DeserializeOwned};
use stellar_rpc_client::Client;
pub use stellar_rpc_client::{
    EventStart, EventType, GetEventsResponse, GetNetworkResponse, GetTransactionResponse,
    SendTransactionResponse, SimulateTransactionResponse, TopicFilter,
};

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("invalid RPC configuration")]
    Configuration,
    #[error("RPC network passphrase does not match configuration")]
    NetworkMismatch,
    #[error("RPC transport temporarily unavailable")]
    Unavailable,
    #[error("RPC rejected the request")]
    Rejected,
    #[error("invalid RPC response")]
    InvalidResponse,
    #[error("unscripted fake RPC call: {0}")]
    Unscripted(String),
}

#[derive(Clone, Debug)]
pub struct EventsRequest {
    pub start: EventStart,
    pub event_type: Option<EventType>,
    pub contract_ids: Vec<String>,
    pub topics: Vec<TopicFilter>,
    pub limit: Option<usize>,
}

/// Internal interface shared by live clients and deterministic, network-free fakes.
#[async_trait]
pub trait SorobanRpc: Send + Sync {
    async fn get_network(&self) -> Result<GetNetworkResponse, RpcError>;
    async fn load_account(&self, address: &str) -> Result<Option<AccountEntry>, RpcError>;
    async fn simulate_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<SimulateTransactionResponse, RpcError>;
    async fn send_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<SendTransactionResponse, RpcError>;
    async fn get_transaction(&self, hash: &Hash) -> Result<GetTransactionResponse, RpcError>;
    async fn get_events(&self, request: &EventsRequest) -> Result<GetEventsResponse, RpcError>;
}

pub struct SorobanRpcClient {
    client: Client,
    passphrase: String,
    retries: u32,
    backoff: Duration,
    timeout: Duration,
}

impl SorobanRpcClient {
    pub fn from_settings(settings: &config::Settings) -> Result<Self, RpcError> {
        Self::new(
            &settings.soroban_rpc_url,
            &settings.stellar_network_passphrase,
            settings.soroban_rpc_timeout_secs,
            settings.soroban_rpc_max_retries,
        )
    }

    pub fn new(
        endpoint: &str,
        passphrase: &str,
        timeout_secs: u64,
        retries: u32,
    ) -> Result<Self, RpcError> {
        let url = url::Url::parse(endpoint).map_err(|_| RpcError::Configuration)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || passphrase.is_empty()
            || timeout_secs == 0
            || retries > 5
        {
            return Err(RpcError::Configuration);
        }
        Ok(Self {
            client: Client::new(endpoint).map_err(|_| RpcError::Configuration)?,
            passphrase: passphrase.to_owned(),
            retries,
            timeout: Duration::from_secs(timeout_secs),
            backoff: Duration::from_millis(100),
        })
    }

    async fn retry<T, F, Fut>(&self, mut operation: F) -> Result<T, RpcError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T, stellar_rpc_client::Error>>,
    {
        for attempt in 0..=self.retries {
            let result =
                tokio::time::timeout(self.timeout, operation()).await.unwrap_or_else(|_| {
                    Err(stellar_rpc_client::Error::JsonRpc(
                        jsonrpsee_core::ClientError::RequestTimeout,
                    ))
                });
            match result {
                Ok(value) => return Ok(value),
                Err(error) => {
                    let transient = match &error {
                        stellar_rpc_client::Error::JsonRpc(
                            jsonrpsee_core::ClientError::RequestTimeout,
                        ) => true,
                        stellar_rpc_client::Error::JsonRpc(
                            jsonrpsee_core::ClientError::Transport(error),
                        ) => {
                            match error.downcast_ref::<jsonrpsee_http_client::transport::Error>() {
                                Some(jsonrpsee_http_client::transport::Error::Rejected {
                                    status_code,
                                }) => {
                                    *status_code == 408
                                        || *status_code == 429
                                        || *status_code >= 500
                                }
                                Some(jsonrpsee_http_client::transport::Error::Http(_)) => true,
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                    if !transient {
                        return Err(RpcError::Rejected);
                    }
                    if attempt == self.retries {
                        return Err(RpcError::Unavailable);
                    }
                    tracing::warn!(attempt, "transient Soroban RPC failure; retrying");
                    let jitter = Duration::from_millis(rand::random::<u64>() % 50);
                    tokio::time::sleep(self.backoff * (1 << attempt) + jitter).await;
                }
            }
        }
        unreachable!("bounded retry loop always returns")
    }
}

#[async_trait]
impl SorobanRpc for SorobanRpcClient {
    #[tracing::instrument(skip_all)]
    async fn get_network(&self) -> Result<GetNetworkResponse, RpcError> {
        let network = self.retry(|| self.client.get_network()).await?;
        if network.passphrase != self.passphrase {
            return Err(RpcError::NetworkMismatch);
        }
        Ok(network)
    }

    #[tracing::instrument(skip_all)]
    async fn load_account(&self, address: &str) -> Result<Option<AccountEntry>, RpcError> {
        self.retry(|| async {
            match self.client.get_account(address).await {
                Ok(account) => Ok(Some(account)),
                Err(stellar_rpc_client::Error::NotFound(..)) => Ok(None),
                Err(error) => Err(error),
            }
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn simulate_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<SimulateTransactionResponse, RpcError> {
        self.retry(|| self.client.simulate_transaction_envelope(tx, None)).await
    }

    #[tracing::instrument(skip_all)]
    async fn send_transaction(
        &self,
        tx: &TransactionEnvelope,
    ) -> Result<SendTransactionResponse, RpcError> {
        // Use the official transport directly: Client::send_transaction discards
        // transport error types and submission status, both needed by callers.
        let encoded = tx.to_xdr_base64(Limits::none()).map_err(|_| RpcError::Rejected)?;
        self.retry(|| async {
            let mut params = ObjectParams::new();
            params.insert("transaction", &encoded)?;
            Ok(self.client.client().request("sendTransaction", params).await?)
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn get_transaction(&self, hash: &Hash) -> Result<GetTransactionResponse, RpcError> {
        self.retry(|| self.client.get_transaction(hash)).await
    }

    #[tracing::instrument(skip_all)]
    async fn get_events(&self, request: &EventsRequest) -> Result<GetEventsResponse, RpcError> {
        if request.limit.is_some_and(|limit| limit == 0 || limit > 10_000) {
            return Err(RpcError::Rejected);
        }
        self.retry(|| {
            self.client.get_events(
                request.start.clone(),
                request.event_type,
                &request.contract_ids,
                &request.topics,
                request.limit,
            )
        })
        .await
    }
}

type FakeResponses = HashMap<String, VecDeque<Result<serde_json::Value, RpcError>>>;

/// Script each method's responses; unconfigured calls fail rather than hitting a network.
#[derive(Default)]
pub struct FakeSorobanRpcClient {
    responses: Mutex<FakeResponses>,
    calls: Mutex<Vec<String>>,
}

impl FakeSorobanRpcClient {
    pub fn push_response<T: Serialize>(&self, method: &str, response: &T) {
        self.responses
            .lock()
            .unwrap()
            .entry(method.to_owned())
            .or_default()
            .push_back(Ok(serde_json::to_value(response).expect("serializable fake response")));
    }
    pub fn push_error(&self, method: &str, error: RpcError) {
        self.responses.lock().unwrap().entry(method.to_owned()).or_default().push_back(Err(error));
    }
    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
    fn response<T: DeserializeOwned>(&self, method: &str) -> Result<T, RpcError> {
        self.calls.lock().unwrap().push(method.to_owned());
        let value = self
            .responses
            .lock()
            .unwrap()
            .get_mut(method)
            .and_then(VecDeque::pop_front)
            .ok_or_else(|| RpcError::Unscripted(method.to_owned()))??;
        serde_json::from_value(value).map_err(|_| RpcError::InvalidResponse)
    }
}

#[async_trait]
impl SorobanRpc for FakeSorobanRpcClient {
    async fn get_network(&self) -> Result<GetNetworkResponse, RpcError> {
        self.response("getNetwork")
    }
    async fn load_account(&self, _: &str) -> Result<Option<AccountEntry>, RpcError> {
        self.response("loadAccount")
    }
    async fn simulate_transaction(
        &self,
        _: &TransactionEnvelope,
    ) -> Result<SimulateTransactionResponse, RpcError> {
        self.response("simulateTransaction")
    }
    async fn send_transaction(
        &self,
        _: &TransactionEnvelope,
    ) -> Result<SendTransactionResponse, RpcError> {
        self.response("sendTransaction")
    }
    async fn get_transaction(&self, _: &Hash) -> Result<GetTransactionResponse, RpcError> {
        self.response("getTransaction")
    }
    async fn get_events(&self, _: &EventsRequest) -> Result<GetEventsResponse, RpcError> {
        self.response("getEvents")
    }
}

#[cfg(test)]
mod tests;
