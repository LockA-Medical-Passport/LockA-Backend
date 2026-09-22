use super::*;
use crate::xdr::*;
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const NETWORK: &str = "Standalone Network ; February 2017";
struct Mock {
    failures: usize,
    failure_status: u16,
    count: AtomicUsize,
    requests: Mutex<Vec<Value>>,
    rpc_error: bool,
    wrong_network: bool,
}
async fn handle(
    State(state): State<Arc<Mock>>,
    Json(request): Json<Value>,
) -> axum::response::Response {
    state.requests.lock().unwrap().push(request.clone());
    if state.count.fetch_add(1, Ordering::SeqCst) < state.failures {
        return StatusCode::from_u16(state.failure_status).unwrap().into_response();
    }
    if state.rpc_error {
        return Json(json!({"jsonrpc":"2.0","id":request["id"],"error":{"code":-32602,"message":"invalid params"}})).into_response();
    }
    let result = match request["method"].as_str().unwrap() {
        "getNetwork" => {
            json!({"passphrase":if state.wrong_network {"wrong network"} else {NETWORK},"protocolVersion":25})
        }
        "getLedgerEntries" => json!({"entries":[],"latestLedger":123}),
        "simulateTransaction" => json!({"latestLedger":123,"minResourceFee":"100"}),
        "sendTransaction" => {
            json!({"hash":"00".repeat(32),"status":"PENDING","latestLedger":123,"latestLedgerCloseTime":"1"})
        }
        "getTransaction" => json!({"status":"NOT_FOUND"}),
        "getEvents" => {
            json!({"events":[],"latestLedger":123,"latestLedgerCloseTime":"1","oldestLedger":1,"oldestLedgerCloseTime":"0","cursor":"cursor-1"})
        }
        method => panic!("unexpected method: {method}"),
    };
    Json(json!({"jsonrpc":"2.0","id":request["id"],"result":result})).into_response()
}
async fn server(
    failures: usize,
    status: u16,
    rpc_error: bool,
    wrong_network: bool,
) -> (SorobanRpcClient, Arc<Mock>, tokio::task::JoinHandle<()>) {
    let state = Arc::new(Mock {
        failures,
        failure_status: status,
        count: AtomicUsize::new(0),
        requests: Mutex::default(),
        rpc_error,
        wrong_network,
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new().route("/", post(handle)).with_state(state.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let mut client = SorobanRpcClient::new(&endpoint, NETWORK, 1, 2).unwrap();
    client.backoff = std::time::Duration::from_millis(1);
    (client, state, task)
}
fn transaction() -> TransactionEnvelope {
    TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: Transaction {
            source_account: MuxedAccount::Ed25519(Uint256([1; 32])),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: Default::default(),
            ext: TransactionExt::V0,
        },
        signatures: Default::default(),
    })
}

#[tokio::test]
async fn live_wrapper_encodes_all_methods_and_preserves_status_and_cursor() {
    let (client, state, task) = server(0, 503, false, false).await;
    assert_eq!(client.get_network().await.unwrap().passphrase, NETWORK);
    let address = stellar_strkey::ed25519::PublicKey([1; 32]).to_string();
    assert!(client.load_account(&address).await.unwrap().is_none());
    let tx = transaction();
    assert_eq!(client.simulate_transaction(&tx).await.unwrap().min_resource_fee, 100);
    assert_eq!(client.send_transaction(&tx).await.unwrap().status, "PENDING");
    assert_eq!(client.get_transaction(&Hash([0; 32])).await.unwrap().status, "NOT_FOUND");
    let events = client
        .get_events(&EventsRequest {
            start: EventStart::Cursor("previous".into()),
            event_type: Some(EventType::Contract),
            contract_ids: vec!["contract".into()],
            topics: vec![],
            limit: Some(10),
        })
        .await
        .unwrap();
    assert_eq!(events.cursor, "cursor-1");
    let requests = state.requests.lock().unwrap();
    assert_eq!(requests[2]["params"]["transaction"], tx.to_xdr_base64(Limits::none()).unwrap());
    assert_eq!(requests[5]["params"]["pagination"]["cursor"], "previous");
    assert!(requests[5]["params"].get("startLedger").is_none());
    task.abort();
}

#[tokio::test]
async fn retries_transient_http_errors_but_bounds_attempts() {
    let (client, state, task) = server(2, 503, false, false).await;
    client.get_network().await.unwrap();
    assert_eq!(state.count.load(Ordering::SeqCst), 3);
    task.abort();
    let (client, state, task) = server(99, 429, false, false).await;
    assert!(matches!(client.send_transaction(&transaction()).await, Err(RpcError::Unavailable)));
    assert_eq!(state.count.load(Ordering::SeqCst), 3);
    task.abort();
}

#[tokio::test]
async fn does_not_retry_permanent_http_or_rpc_errors_or_network_mismatch() {
    for (failures, status, rpc_error, wrong_network) in
        [(99, 400, false, false), (0, 200, true, false), (0, 200, false, true)]
    {
        let (client, state, task) = server(failures, status, rpc_error, wrong_network).await;
        assert!(client.get_network().await.is_err());
        assert_eq!(state.count.load(Ordering::SeqCst), 1);
        task.abort();
    }
}

#[tokio::test]
async fn fake_is_usable_through_the_same_interface_without_network() {
    let fake = FakeSorobanRpcClient::default();
    fake.push_response("getNetwork", &json!({"passphrase":NETWORK,"protocolVersion":25}));
    fake.push_response("loadAccount", &Option::<AccountEntry>::None);
    fake.push_response("simulateTransaction", &SimulateTransactionResponse::default());
    fake.push_error("sendTransaction", RpcError::Unavailable);
    let client: &dyn SorobanRpc = &fake;
    client.get_network().await.unwrap();
    assert!(client.load_account("unused").await.unwrap().is_none());
    client.simulate_transaction(&transaction()).await.unwrap();
    assert!(matches!(client.send_transaction(&transaction()).await, Err(RpcError::Unavailable)));
    assert!(matches!(client.get_network().await, Err(RpcError::Unscripted(_))));
    assert_eq!(fake.calls().len(), 5);
}

#[tokio::test]
#[ignore = "requires a running Stellar quickstart and funded SMOKE_ACCOUNT"]
async fn local_network_simulates_a_noop_without_submitting_it() {
    let endpoint = std::env::var("SOROBAN_RPC_URL").expect("SOROBAN_RPC_URL");
    let passphrase =
        std::env::var("STELLAR_NETWORK_PASSPHRASE").expect("STELLAR_NETWORK_PASSPHRASE");
    let address = std::env::var("SMOKE_ACCOUNT").expect("SMOKE_ACCOUNT");
    let client = SorobanRpcClient::new(&endpoint, &passphrase, 20, 3).unwrap();
    client.get_network().await.unwrap();
    let account = client
        .load_account(&address)
        .await
        .unwrap()
        .expect("fund the smoke account with friendbot");
    // Empty-footprint ExtendFootprintTtl is a Soroban operation with no
    // application state changes. Simulation never submits a transaction.
    let tx = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(
                stellar_strkey::ed25519::PublicKey::from_string(&address).unwrap().0,
            )),
            fee: 100,
            seq_num: SequenceNumber(account.seq_num.0 + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::ExtendFootprintTtl(ExtendFootprintTtlOp {
                    ext: ExtensionPoint::V0,
                    extend_to: 100,
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: SorobanTransactionDataExt::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: Default::default(),
                        read_write: Default::default(),
                    },
                    instructions: 0,
                    disk_read_bytes: 0,
                    write_bytes: 0,
                },
                resource_fee: 0,
            }),
        },
        signatures: Default::default(),
    });
    let result = client.simulate_transaction(&tx).await.unwrap();
    assert!(result.error.is_none(), "simulation failed: {:?}", result.error);
}
