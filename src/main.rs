mod bundler;
mod consts;
mod traits;
mod types;

use anyhow::Ok;
use bundler::run_bundler;
use clap::{Parser, Subcommand};
use consts::{GETH_CHAIN_ID, GETH_ENTRY_POINT_ADDRESS};
use ethers::types::Address;
use ethers_userop::types::{BundlerConfig, RpcConfig, UserOperationPoolConfig};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;

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

                Ok(())
            }
            BundlerSubcommands::RunTest {} => {
                run_bundler(
                    BundlerConfig::default(),
                    RpcConfig::default(),
                    UserOperationPoolConfig::default(),
                    "http://localhost:8545".to_string(),
                    GETH_CHAIN_ID,
                    GETH_ENTRY_POINT_ADDRESS.parse::<Address>().unwrap(),
                    true,
                )
                .await?;
                Ok(())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    bundler_config: BundlerJsonConfig,
    wallet_config: WalletJsonConfig,
}

#[derive(Parser, Serialize, Deserialize, Debug)]
struct BundlerJsonConfig {}

#[derive(Parser, Serialize, Deserialize, Debug)]
struct WalletJsonConfig {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();
    let _ = match opts.sub {
        Subcommands::Bundler { command } => {
            command.run().await?;
            Ok(())
        }
        Subcommands::Wallet { command } => {
            command.run().await?;
            Ok(())
        }
    };
    Ok(())
}
