use dotenv::dotenv;
use env_logger::Env;
use ethers::{
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::{Http, Middleware, Provider},
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::{Address, TransactionRequest, U256},
    utils::{Geth, GethInstance},
};
use ethers_userop::{
    gen::{EntryPoint, SimpleAccountFactory},
    types::{ClientType, DeployedContract, GETH_ENTRY_POINT_ADDRESS, RPC_NAMESPACE, SEED_PHRASE},
};
use hashbrown::HashSet;
use pin_utils::pin_mut;
use silius_grpc::{
    bundler_client::BundlerClient, bundler_service_run, uo_pool_client::UoPoolClient,
    uopool_service_run,
};
use silius_primitives::{Chain, UoPoolMode, Wallet};
use silius_rpc::{
    debug_api::{DebugApiServer, DebugApiServerImpl},
    eth_api::{EthApiServer, EthApiServerImpl},
    JsonRpcServer,
};
use std::{
    env,
    future::{pending, Future},
    panic,
    sync::Arc,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Mul,
    time::Duration,
};
use tempdir::TempDir;
use tracing::{info, warn};

// Based on https://github.com/Vid201/silius/blob/main/bin/silius/src/silius.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        Env::default().default_filter_or("info"), //.default_filter_or("trace")
    )
    .init();

    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt::init();
    }

    dotenv().ok();
    let (_geth, client) = setup_geth().await?;
    let client = Arc::new(client);
    let entry_point = deploy_entry_point(client.clone()).await?;
    let _simple_account_factory =
        deploy_simple_account_factory(client.clone(), entry_point.address).await?;

    let eth_client_address = "http://localhost:8545".to_string();

    std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(128 * 1024 * 1024)
                .build()?;

            let task = async move {
                info!("Starting Silius Bundler");

                let eth_client = Arc::new(Provider::<Http>::try_from(eth_client_address.clone())?);
                info!(
                    "Connected to the Ethereum execution client at {}: {}",
                    eth_client_address.clone(),
                    eth_client.client_version().await?
                );

                let chain_id = eth_client.get_chainid().await?;
                let chain = Chain::from(chain_id);

                let wallet_path_option = env::var("WALLET_PATH");
                let wallet = match wallet_path_option {
                    Ok(wallet_path) => {
                        let full_path = format!("{}/{}", env::var("HOME").unwrap(), wallet_path);
                        Wallet::from_file(full_path.into(), &U256::from(5))
                    }
                    Err(_) => {
                        warn!(
                            "WALLET_PATH not set in .env. Wallet will use default test SEED_PHRASE"
                        );
                        Err(anyhow::anyhow!("WALLET_PATH not set"))
                    }
                }
                .unwrap_or_else(|_| {
                    Wallet::from_phrase(SEED_PHRASE, &U256::from(chain.id())).unwrap()
                });
                info!("{:?}", &wallet.signer);

                let uopool_grpc_listen_address: SocketAddr =
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3001);
                info!("Starting uopool gRPC service...");
                uopool_service_run(
                    uopool_grpc_listen_address,
                    vec![GETH_ENTRY_POINT_ADDRESS.clone().parse::<Address>()?],
                    eth_client,
                    chain,
                    U256::from(1500000),
                    U256::from(1),
                    U256::zero(),
                    U256::zero(),
                    vec![],
                    UoPoolMode::Standard,
                )
                .await?;
                info!("Started uopool gRPC service at {:}", "127.0.0.1:3001");

                info!("Connecting to uopool gRPC service");
                let uopool_grpc_client =
                    UoPoolClient::connect(format!("http://{}", uopool_grpc_listen_address)).await?;
                info!("Connected to uopool gRPC service");

                info!("Starting bundler gRPC service...");
                bundler_service_run(
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3002),
                    wallet.clone(),
                    vec![GETH_ENTRY_POINT_ADDRESS.clone().parse::<Address>()?],
                    eth_client_address.clone(),
                    chain,
                    wallet.signer.address(),
                    U256::from(600),
                    U256::from(1),
                    10,
                    uopool_grpc_client.clone(),
                );
                info!("Started bundler gRPC service at {:}", "127.0.0.1:3002");

                info!("Starting bundler JSON-RPC server...");
                tokio::spawn({
                    async move {
                        let _api: HashSet<String> = HashSet::from_iter(
                            RPC_NAMESPACE
                                .map(|s| s.to_string())
                                .into_iter()
                                .collect::<Vec<String>>(),
                        );

                        let mut server = JsonRpcServer::new("127.0.0.1:3000".to_string())
                            .with_proxy(eth_client_address.to_string())
                            .with_cors(vec!["*".to_string()]);

                        // web3 namespace is ignored
                        // if api.contains("web3") {
                        //     server.add_method(Web3ApiServerImpl{}.into_rpc())?;
                        // }

                        server.add_method(
                            EthApiServerImpl {
                                uopool_grpc_client: uopool_grpc_client.clone(),
                            }
                            .into_rpc(),
                        )?;

                        let bundler_grpc_client = BundlerClient::connect(format!(
                            "http://{}",
                            uopool_grpc_listen_address
                        ))
                        .await?;
                        server.add_method(
                            DebugApiServerImpl {
                                uopool_grpc_client,
                                bundler_grpc_client,
                            }
                            .into_rpc(),
                        )?;

                        let _handle = server.start().await?;
                        info!("Started bundler JSON-RPC server at {:}", "127.0.0.1:3000");

                        pending::<anyhow::Result<()>>().await
                    }
                });
                pending::<anyhow::Result<()>>().await
            };
            rt.block_on(run_until_ctrl_c::<_, anyhow::Error>(task))?;
            Ok(())
        })?
        .join()
        .unwrap_or_else(|e| panic::resume_unwind(e))
}

/// Runs the future to completion or until:
/// - `ctrl-c` is received.
/// - `SIGTERM` is received (unix only).
async fn run_until_ctrl_c<F, E>(fut: F) -> anyhow::Result<(), E>
where
    F: Future<Output = Result<(), E>>,
    E: Send + Sync + 'static + From<std::io::Error>,
{
    let ctrl_c = tokio::signal::ctrl_c();

    let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let sigterm = stream.recv();
    pin_mut!(sigterm, ctrl_c, fut);

    tokio::select! {
        _ = ctrl_c => {
            info!("Received ctrl-c signal.");
        },
        _ = sigterm => {
            info!("Received SIGTERM signal.");
        },
        res = fut => res?,
    }

    Ok(())
}

// Based on https://github.com/Vid201/silius/blob/main/tests/src/common/mod.rs
pub async fn setup_geth() -> anyhow::Result<(GethInstance, ClientType)> {
    let chain_id: u64 = 1337;
    let tmp_dir = TempDir::new("test_geth")?;
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(SEED_PHRASE)
        .build()?;

    let port = 8545u16;
    let geth = Geth::new()
        .data_dir(tmp_dir.path().to_path_buf())
        .port(port)
        .spawn();

    let provider =
        Provider::<Http>::try_from(geth.endpoint())?.interval(Duration::from_millis(10u64));

    let client = SignerMiddleware::new(provider.clone(), wallet.clone().with_chain_id(chain_id))
        .nonce_manager(wallet.address());

    let coinbase = client.get_accounts().await?[0];
    let tx = TransactionRequest::new()
        .to(wallet.address())
        .value(U256::from(10).pow(U256::from(18)).mul(100))
        .from(coinbase);
    provider.send_transaction(tx, None).await?.await?;
    Ok((geth, client))
}

pub async fn deploy_entry_point<M: Middleware + 'static>(
    client: Arc<M>,
) -> anyhow::Result<DeployedContract<EntryPoint<M>>> {
    let (ep, receipt) = EntryPoint::deploy(client, ())?.send_with_receipt().await?;
    let addr = receipt.contract_address.unwrap_or(Address::zero());
    Ok(DeployedContract::new(ep, addr))
}

pub async fn deploy_simple_account_factory<M: Middleware + 'static>(
    client: Arc<M>,
    ep_addr: Address,
) -> anyhow::Result<DeployedContract<SimpleAccountFactory<M>>> {
    let (saf, receipt) = SimpleAccountFactory::deploy(client, ep_addr)?
        .send_with_receipt()
        .await?;
    let addr = receipt.contract_address.unwrap_or(Address::zero());
    Ok(DeployedContract::new(saf, addr))
}
