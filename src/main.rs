#[allow(dead_code)]
mod bundler;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod consts;
#[allow(dead_code)]
mod gen;
#[allow(dead_code)]
mod traits;
#[allow(dead_code)]
mod types;

use anyhow::Ok;
use clap::Parser;
use config::Opts;
use config::Subcommands;

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
