# Ethers UserOp
An ether-rs middleware to craft UserOperations

## Pre-requisites
[Geth](https://geth.ethereum.org/docs/getting-started/installing-geth) (tested with v1.12.2).

## Use
To start a [Silius](https://github.com/Vid201/silius) bundler with user operation pool and JSON-RPC API with default config at `127.0.0.1:3000` on [Geth Testnet](https://chainlist.org/chain/1337)
```bash
cargo run --bin ethuo bundler test
```
To generate a random key and address(generated key will be updated in `config.json`)
```bash
cargo run --bin ethuo wallet new-key
```
To generate a counter-factual address(generated address will be updated in `config.json`)
```bash
cargo run --bin ethuo wallet new-wallet-address
```

To use `UserOpMiddleware` in your code
```rust
use ethers::{
    contract::abigen,
    providers::{Http, Provider},
    signers::Signer,
    types::{Address, U256},
};
use ethers_userop::{
    consts::{GETH_CHAIN_ID, GETH_ENTRY_POINT_ADDRESS, GETH_WETH_ADDRESS, SALT, SEED_PHRASE},
    UserOpMiddleware,
};
use silius_primitives::Wallet as UoWallet;
use std::thread;
use std::time::Duration;

abigen!(WETH, "src/abi/WETH.json",);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .init();
    // Setup the environment
    let eth_client_address = "http://localhost:8545".to_string();
    let seed = SEED_PHRASE.to_string();
    let provider = Provider::<Http>::try_from(eth_client_address.to_string())?;
    let rpc_address = format!("http://{}", "127.0.0.1:3000");
    let chain_id = GETH_CHAIN_ID; // Geth testnet
    let uo_wallet = UoWallet::from_phrase(seed.as_str(), &U256::from(chain_id), false).unwrap();

    let signer_wallet_address = uo_wallet.clone().signer.address();
    let wallet_name = "simple-account-test";

    // Instantiate a UserOpMiddleware
    let mut uo_middleware: UserOpMiddleware<Provider<Http>> = UserOpMiddleware::new(
        provider,
        GETH_ENTRY_POINT_ADDRESS.parse::<Address>().unwrap(),
        rpc_address,
        uo_wallet.clone(),
    );

    // Deploy a smart contract wallet
    let (uo_hash, scw_address) = uo_middleware
        .deploy_scw(wallet_name.into(), 10u64, SALT)
        .await?;
    println!(
        "Smart contract wallet deployed at {:x} 
        : {:?}",
        scw_address, uo_hash
    );

    // Force to wait for the smart contract wallet to be deployed on the next block
    thread::sleep(Duration::from_secs(12));

    // Send Eth
    let uo_hash = uo_middleware
        .send_eth(scw_address, wallet_name, signer_wallet_address, 1u64)
        .await?;
    println!("Sent ETH to {}: {:?}", signer_wallet_address, uo_hash);

    // Force to wait for the smart contract wallet to be deployed on the next block
    thread::sleep(Duration::from_secs(12));

    // Calling Weth contract to deposit Eth
    let weth = WETH::new(
        GETH_WETH_ADDRESS.parse::<Address>()?,
        uo_middleware.clone().into(),
    );

    let mut deposit = weth.deposit().tx;
    deposit.set_value(U256::from(100u64));
    let uo_hash = uo_middleware
        .call(wallet_name, scw_address, deposit)
        .await?;
    println!("Deposited ETH into WETH contract: {:?}", uo_hash);

    Ok(())
}
```
