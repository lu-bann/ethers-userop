#[allow(dead_code)]
mod bundler;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod consts;
#[allow(dead_code)]
mod errors;
#[allow(dead_code)]
mod gen;
mod traits;
#[allow(dead_code)]
mod types;
#[allow(dead_code)]
mod uo_builder;
#[allow(dead_code)]
mod userop_middleware;

use crate::bundler::run_until_ctrl_c;
use anyhow::Ok;
use clap::Parser;
use config::Opts;
use config::Subcommands;
use std::panic;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    std::env::set_var("RUST_LOG", format!("ethuo={}", opts.get_log_level()));
    tracing_subscriber::fmt::init();

    std::thread::Builder::new()
        .stack_size(128 * 1024 * 1024)
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(128 * 1024 * 1024)
                .build()?;

            let task = async move {
                match opts.sub {
                    Subcommands::Bundler { command } => {
                        command.run().await?;
                        Ok(())
                    }
                    Subcommands::Wallet { command } => {
                        command.run().await?;
                        Ok(())
                    }
                }
            };

            rt.block_on(run_until_ctrl_c(task))?;
            Ok(())
        })?
        .join()
        .unwrap_or_else(|e| panic::resume_unwind(e))
}
