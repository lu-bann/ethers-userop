use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(name = "ethuo")]
pub struct Opts {
    #[clap(subcommand)]
    pub sub: Subcommands,
}

#[derive(Debug, Subcommand)]
pub enum Subcommands {
    /// Wallet management utilities.
    #[clap(visible_alias = "w")]
    Wallet {
        #[clap(subcommand)]
        command: WalletSubcommands,
    },
    /// Bundler management utilities.
    #[clap(visible_alias = "b")]
    Bundler {
        #[clap(subcommand)]
        command: BundlerSubcommands,
    },
}

#[derive(Debug, Parser)]
pub enum WalletSubcommands {}

#[derive(Debug, Parser)]
pub enum BundlerSubcommands {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    todo!()
}
