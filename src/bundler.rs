use crate::{
    config::{BundlerConfig, RpcConfig, UserOperationPoolConfig},
    consts::{GETH_ENTRY_POINT_ADDRESS, RPC_NAMESPACE, SEED_PHRASE},
    gen::{EntryPoint, SimpleAccountFactory},
    types::{DeployedContract, SignerType},
};
use dirs::home_dir;
use dotenv::dotenv;
use ethers::{
    contract::abigen,
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::{Http, Middleware, Provider},
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::{Address, TransactionRequest, U256},
    utils::{Geth, GethInstance},
};
use expanded_pathbuf::ExpandedPathBuf;
use hashbrown::HashSet;
use pin_utils::pin_mut;
use silius_grpc::{
    bundler_client::BundlerClient, bundler_service_run, uo_pool_client::UoPoolClient,
    uopool_service_run,
};
use silius_primitives::{
    bundler::SendBundleMode, consts::flashbots_relay_endpoints, Chain, UoPoolMode, Wallet,
};
use silius_rpc::{
    eth_api::{EthApiServer, EthApiServerImpl},
    JsonRpcServer, JsonRpcServerType,
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

abigen!(WETH, "src/abi/WETH.json",);
abigen!(ERC20, "src/abi/ERC20.json",);

pub async fn run_bundler(
    bundler_config: BundlerConfig,
    rpc_config: RpcConfig,
    uopool_config: UserOperationPoolConfig,
    eth_client_address: String,
    entry_point_address: Address,
) -> anyhow::Result<()> {
    // Bundler configs
    let bundler_address = bundler_config.bundler_address;
    let bundler_port = bundler_config.bundler_port;
    let bundler_seed = bundler_config.bundler_seed;
    let beneficiary_address = bundler_config.beneficiary_address;
    let min_balance = U256::from(bundler_config.min_balance);
    let bundle_interval = bundler_config.bundle_interval;
    let send_bundle_mode = bundler_config.send_bundle_mode;

    // UserOperationPool configs
    let uo_pool_address = uopool_config.uo_pool_address;
    let uo_pool_port = uopool_config.uo_pool_port;
    let max_verification_gas = uopool_config.max_verification_gas;
    let min_stake = uopool_config.min_stake;
    let min_unstake_delay = uopool_config.min_unstake_delay;
    let min_priority_fee_per_gas = uopool_config.min_priority_fee_per_gas;
    let whitelist = uopool_config.whitelist;
    let uopool_mode = uopool_config.uopool_mode;

    // RPC configs
    let http = rpc_config.http;
    let http_addr = rpc_config.http_addr;
    let http_port = rpc_config.http_port;
    let ws = rpc_config.ws;
    let _ws_addr = rpc_config.ws_addr;
    let ws_port = rpc_config.ws_port;

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

                let fb_or_eth = match send_bundle_mode {
                    SendBundleMode::EthClient => false,
                    SendBundleMode::Flashbots => true,
                };
                let wallet = Wallet::from_phrase(&bundler_seed, &chain_id, fb_or_eth)?;

                let datadir = home_dir()
                    .map(|h| h.join(".silius"))
                    .ok_or_else(|| anyhow::anyhow!("Get Home directory error"))
                    .map(ExpandedPathBuf)?;

                let uopool_grpc_listen_address: SocketAddr =
                    SocketAddr::new(uo_pool_address, uo_pool_port);
                info!("Starting uopool gRPC service...");
                uopool_service_run(
                    uopool_grpc_listen_address,
                    datadir,
                    vec![entry_point_address],
                    eth_client,
                    chain,
                    max_verification_gas.into(),
                    min_stake.into(),
                    min_unstake_delay.into(),
                    min_priority_fee_per_gas.into(),
                    whitelist,
                    uopool_mode,
                )
                .await?;
                info!(
                    "Started uopool gRPC service at {:}",
                    uopool_grpc_listen_address
                );

                info!("Connecting to uopool gRPC service");
                let uopool_grpc_client =
                    UoPoolClient::connect(format!("http://{}", uopool_grpc_listen_address)).await?;
                info!("Connected to uopool gRPC service");

                let bundler_grpc_listen_address: SocketAddr =
                    SocketAddr::new(bundler_address, bundler_port);
                info!("Starting bundler gRPC service...");
                bundler_service_run(
                    bundler_grpc_listen_address,
                    wallet.clone(),
                    vec![entry_point_address],
                    eth_client_address.clone(),
                    chain,
                    beneficiary_address,
                    min_balance,
                    bundle_interval,
                    uopool_grpc_client.clone(),
                    send_bundle_mode,
                    match send_bundle_mode {
                        SendBundleMode::EthClient => None,
                        SendBundleMode::Flashbots => {
                            Some(vec![flashbots_relay_endpoints::FLASHBOTS.to_string()])
                        }
                    },
                );
                info!(
                    "Started bundler gRPC service at {:}",
                    bundler_grpc_listen_address
                );

                info!("Starting bundler JSON-RPC server...");
                tokio::spawn({
                    async move {
                        let _api: HashSet<String> = HashSet::from_iter(
                            RPC_NAMESPACE
                                .map(|s| s.to_string())
                                .into_iter()
                                .collect::<Vec<String>>(),
                        );

                        let mut server = JsonRpcServer::new(
                            http,
                            IpAddr::V4(Ipv4Addr::LOCALHOST),
                            http_port,
                            ws,
                            IpAddr::V4(Ipv4Addr::LOCALHOST),
                            ws_port,
                        )
                        .with_proxy(eth_client_address.to_string())
                        .with_cors(&vec!["*".to_string()], JsonRpcServerType::Http)
                        .with_cors(&vec!["*".to_string()], JsonRpcServerType::Ws);

                        server.add_methods(
                            EthApiServerImpl {
                                uopool_grpc_client: uopool_grpc_client.clone(),
                            }
                            .into_rpc(),
                            JsonRpcServerType::Http,
                        )?;

                        let _bundler_grpc_client = BundlerClient::connect(format!(
                            "http://{}",
                            uopool_grpc_listen_address
                        ))
                        .await?;

                        let _handle = server.start().await?;
                        info!(
                            "Started bundler JSON-RPC server at {:}:{:}",
                            http_addr, http_port
                        );

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

pub async fn run_bundler_test() -> anyhow::Result<()> {
    dotenv().ok();
    let (_geth, client): (_, SignerType<Provider<Http>>) = setup_geth().await?;
    let client = Arc::new(client);
    let entry_point = deploy_entry_point(client.clone()).await?;
    let _simple_account_factory =
        deploy_simple_account_factory(client.clone(), entry_point.address).await?;
    let _erc20 = deploy_weth(client.clone()).await?;
    let _weth = deploy_weth(client).await?;

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
                        // generate a wallet without flashbots key
                        Wallet::from_file(full_path.into(), &U256::from(5), false)
                    }
                    Err(_) => {
                        warn!(
                            "WALLET_PATH not set in .env. Wallet will use default test SEED_PHRASE"
                        );
                        Err(anyhow::anyhow!("WALLET_PATH not set"))
                    }
                }
                .unwrap_or_else(|_| {
                    Wallet::from_phrase(SEED_PHRASE, &U256::from(chain.id()), false).unwrap()
                });
                info!("{:?}", &wallet.signer);

                let datadir = home_dir()
                    .map(|h| h.join(".silius"))
                    .ok_or_else(|| anyhow::anyhow!("Get Home directory error"))
                    .map(ExpandedPathBuf)?;

                let uopool_grpc_listen_address: SocketAddr =
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 3001);
                info!("Starting uopool gRPC service...");
                uopool_service_run(
                    uopool_grpc_listen_address,
                    datadir,
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
                    U256::from(1),
                    10,
                    uopool_grpc_client.clone(),
                    SendBundleMode::EthClient,
                    None, // TODO: add flashbots relay
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

                        let mut server = JsonRpcServer::new(
                            true,
                            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                            3000,
                            false,
                            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
                            3001,
                        )
                        .with_proxy(eth_client_address.to_string())
                        .with_cors(&vec!["*".to_string()], JsonRpcServerType::Http)
                        .with_cors(&vec!["*".to_string()], JsonRpcServerType::Ws);

                        // web3 namespace is ignored
                        // if api.contains("web3") {
                        //     server.add_method(Web3ApiServerImpl{}.into_rpc())?;
                        // }

                        server.add_methods(
                            EthApiServerImpl {
                                uopool_grpc_client: uopool_grpc_client.clone(),
                            }
                            .into_rpc(),
                            JsonRpcServerType::Http,
                        )?;

                        let _bundler_grpc_client = BundlerClient::connect(format!(
                            "http://{}",
                            uopool_grpc_listen_address
                        ))
                        .await?;

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
pub(crate) async fn run_until_ctrl_c<F, E>(fut: F) -> anyhow::Result<(), E>
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
pub(crate) async fn setup_geth() -> anyhow::Result<(GethInstance, SignerType<Provider<Http>>)> {
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

    let provider = Arc::new(
        Provider::<Http>::try_from(geth.endpoint())?.interval(Duration::from_millis(10u64)),
    );

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

pub(crate) async fn deploy_entry_point<M: Middleware + 'static>(
    client: Arc<M>,
) -> anyhow::Result<DeployedContract<EntryPoint<M>>> {
    let (ep, receipt) = EntryPoint::deploy(client, ())?.send_with_receipt().await?;
    let addr = receipt.contract_address.unwrap_or(Address::zero());
    Ok(DeployedContract::new(ep, addr))
}

pub(crate) async fn deploy_simple_account_factory<M: Middleware + 'static>(
    client: Arc<M>,
    ep_addr: Address,
) -> anyhow::Result<DeployedContract<SimpleAccountFactory<M>>> {
    let (saf, receipt) = SimpleAccountFactory::deploy(client, ep_addr)?
        .send_with_receipt()
        .await?;
    let addr = receipt.contract_address.unwrap_or(Address::zero());
    Ok(DeployedContract::new(saf, addr))
}

pub(crate) async fn deploy_weth<M: Middleware + 'static>(
    client: Arc<M>,
) -> anyhow::Result<DeployedContract<WETH<M>>> {
    let (saf, receipt) = WETH::deploy(client, ())?.send_with_receipt().await?;
    let addr = receipt.contract_address.unwrap_or(Address::zero());
    Ok(DeployedContract::new(saf, addr))
}
#[allow(dead_code)]
pub(crate) async fn deploy_erc20<M: Middleware + 'static>(
    client: Arc<M>,
) -> anyhow::Result<DeployedContract<ERC20<M>>> {
    let (saf, receipt) = ERC20::deploy(client, ())?.send_with_receipt().await?;
    let addr = receipt.contract_address.unwrap_or(Address::zero());
    Ok(DeployedContract::new(saf, addr))
}
