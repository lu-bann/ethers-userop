use crate::bundler::{run_bundler, run_bundler_test};
use anyhow::Ok;
use clap::{Parser, Subcommand};
use ethers::types::Address;
use serde::{Deserialize, Serialize};
use silius_primitives::{bundler::SendBundleMode, uopool::Mode as UoPoolMode};
use std::fs::File;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Debug, Parser)]
#[clap(name = "ethuo")]
pub struct Opts {
    /// Command to execute
    #[clap(subcommand)]
    pub sub: Subcommands,
}

#[derive(Debug, Subcommand)]
pub enum Subcommands {
    /// Wallet management utilities.
    #[clap(name = "wallet", visible_alias = "w")]
    Wallet {
        #[clap(subcommand)]
        command: WalletSubcommands,
    },
    /// Bundler management utilities.
    #[clap(name = "bundler", visible_alias = "b")]
    Bundler {
        #[clap(subcommand)]
        command: BundlerSubcommands,
    },
}

#[derive(Debug, Parser)]
pub enum WalletSubcommands {
    /// Generate a new signing key
    #[clap(name = "new-key", visible_alias = "k")]
    NewKey {},
    /// Generate a counter-factual address
    #[clap(name = "new-wallet-address", visible_alias = "nwa")]
    NewWalletAddress {},
    /// Deploy a new smart contract wallet
    #[clap(name = "new-wallet", visible_alias = "nw")]
    NewWallet {},
    /// Transfer from a smart contract wallet
    #[clap(name = "transfer", visible_alias = "t")]
    Transfer {},
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    json_bundler_config: BundlerJsonConfig,
    json_wallet_config: WalletJsonConfig,
}

#[derive(Serialize, Deserialize, Debug)]
struct BundlerJsonConfig {
    eth_client: String,
    entry_point_address: Address,
    bundler_config: BundlerConfig,
    user_operation_pool_config: UserOperationPoolConfig,
    rpc_config: RpcConfig,
}

#[derive(Parser, Serialize, Deserialize, Debug)]
struct WalletJsonConfig {}

impl WalletSubcommands {
    pub async fn run(&self) -> anyhow::Result<()> {
        match self {
            WalletSubcommands::NewKey {} => Ok(()),
            WalletSubcommands::NewWalletAddress {} => Ok(()),
            WalletSubcommands::NewWallet {} => Ok(()),
            WalletSubcommands::Transfer {} => Ok(()),
        }
    }
}

#[derive(Debug, Parser)]
pub enum BundlerSubcommands {
    /// Run the bundler with configuration from the config.json
    #[clap(name = "run", visible_alias = "r")]
    Run {},
    /// Run the bundler on a local Geth test net
    #[clap(name = "test", visible_alias = "t")]
    RunTest {},
}

impl BundlerSubcommands {
    pub async fn run(&self) -> anyhow::Result<()> {
        match self {
            BundlerSubcommands::Run {} => {
                let mut file = File::open("config.json")?;
                let mut content = String::new();
                file.read_to_string(&mut content)?;
                let config: Config = serde_json::from_str(&content)?;

                let bundler_config = BundlerConfig {
                    bundler_address: config.json_bundler_config.bundler_config.bundler_address,
                    bundler_port: config.json_bundler_config.bundler_config.bundler_port,
                    bundler_seed: config.json_bundler_config.bundler_config.bundler_seed,
                    beneficiary_address: config
                        .json_bundler_config
                        .bundler_config
                        .beneficiary_address,
                    min_balance: config.json_bundler_config.bundler_config.min_balance,
                    bundle_interval: config.json_bundler_config.bundler_config.bundle_interval,
                    send_bundle_mode: config.json_bundler_config.bundler_config.send_bundle_mode,
                };

                let uo_pool_config = UserOperationPoolConfig {
                    uo_pool_address: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .uo_pool_address,
                    uo_pool_port: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .uo_pool_port,
                    max_verification_gas: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .max_verification_gas,
                    min_stake: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .min_stake,
                    min_unstake_delay: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .min_unstake_delay,
                    min_priority_fee_per_gas: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .min_priority_fee_per_gas,
                    whitelist: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .whitelist,
                    uopool_mode: config
                        .json_bundler_config
                        .user_operation_pool_config
                        .uopool_mode,
                };

                let rpc_config = RpcConfig {
                    http: config.json_bundler_config.rpc_config.http,
                    http_port: config.json_bundler_config.rpc_config.http_port,
                    http_addr: config.json_bundler_config.rpc_config.http_addr,
                    ws: config.json_bundler_config.rpc_config.ws,
                    ws_port: config.json_bundler_config.rpc_config.ws_port,
                    ws_addr: config.json_bundler_config.rpc_config.ws_addr,
                };

                let eth_client = config.json_bundler_config.eth_client;
                let entry_point_address = config.json_bundler_config.entry_point_address;

                run_bundler(
                    bundler_config,
                    rpc_config,
                    uo_pool_config,
                    eth_client,
                    entry_point_address,
                )
                .await?;

                Ok(())
            }
            BundlerSubcommands::RunTest {} => {
                run_bundler_test().await?;
                Ok(())
            }
        }
    }
}

/// Bundler config
#[derive(Serialize, Clone, Deserialize, Debug)]
pub struct BundlerConfig {
    /// Bundler gRPC address to listen on.
    pub bundler_address: IpAddr,
    /// Bundler gRPC port to listen on.
    pub bundler_port: u16,
    /// Bundler EOA wallet seed phrase
    pub bundler_seed: String,
    /// The bundler beneficiary address.
    pub beneficiary_address: Address,
    /// The minimum balance required for the beneficiary address.
    pub min_balance: u64,
    /// The bundle interval in seconds.
    pub bundle_interval: u64,
    /// Sets the send bundle mode.
    pub send_bundle_mode: SendBundleMode,
}

impl Default for BundlerConfig {
    fn default() -> Self {
        Self {
            bundler_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            bundler_port: 3002,
            bundler_seed: "test test test test test test test test test test test junk".to_string(),
            beneficiary_address: Address::zero(),
            min_balance: 0u64,
            bundle_interval: 5,
            send_bundle_mode: SendBundleMode::EthClient,
        }
    }
}

/// UserOperationPool config
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UserOperationPoolConfig {
    /// UoPool gRPC address to listen on.
    pub uo_pool_address: IpAddr,
    /// UoPool gRPC port to listen on.
    pub uo_pool_port: u16,
    /// Max allowed verification gas.
    pub max_verification_gas: u64,
    /// Minimum stake required for entities.
    pub min_stake: u64,
    /// Minimum unstake delay for entities.
    pub min_unstake_delay: u64,
    /// Minimum priority fee per gas.
    pub min_priority_fee_per_gas: u64,
    /// Addresses of whitelisted entities.
    pub whitelist: Vec<Address>,
    /// User operation mempool mode
    pub uopool_mode: UoPoolMode,
}

impl Default for UserOperationPoolConfig {
    fn default() -> Self {
        Self {
            uo_pool_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            uo_pool_port: 3003,
            max_verification_gas: 1,
            min_stake: 1,
            min_unstake_delay: 1,
            min_priority_fee_per_gas: 1,
            whitelist: vec![],
            uopool_mode: UoPoolMode::Standard,
        }
    }
}

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct RpcConfig {
    /// Enables or disables the HTTP RPC.
    pub http: bool,
    /// Sets the HTTP RPC address to listen on.
    pub http_addr: String,
    /// Sets the HTTP RPC port to listen on.
    pub http_port: u16,
    /// Enables or disables the WebSocket RPC.
    pub ws: bool,
    /// Sets the WS RPC address to listen on.
    pub ws_addr: String,
    /// Sets the WS RPC port to listen on.
    pub ws_port: u16,
}
