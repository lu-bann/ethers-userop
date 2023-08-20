use ethers::{
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::{Http, Middleware, Provider},
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::{transaction::eip2718::TypedTransaction, Address, U256},
    utils::parse_ether,
};
use ethers_userop::{
    gen::{EntryPoint, SimpleAccountFactory},
    types::{
        GETH_CHAIN_ID, GETH_ENTRY_POINT_ADDRESS, GETH_SIMPLE_ACCOUNT_FACTORY, SALT, SEED_PHRASE,
    },
    UserOpMiddleware,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup the environment
    let eth_client_address = "http://localhost:8545".to_string();
    let seed = SEED_PHRASE.to_string();
    let provider = Arc::new(Provider::<Http>::try_from(eth_client_address.to_string())?);
    let rpc_address = format!("http://{}", "127.0.0.1:3000");
    let chain_id = GETH_CHAIN_ID; // Geth testnet
    let factory_address = GETH_SIMPLE_ACCOUNT_FACTORY.to_string().parse::<Address>()?;
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(seed.clone().as_str())
        .build()?;
    let signer_address = wallet.address();
    let client = SignerMiddleware::new(provider.clone(), wallet.clone().with_chain_id(chain_id))
        .nonce_manager(wallet.clone().address());
    let provider = Arc::new(client);

    // Instantiate the SimpleAccountFactory contract
    let simple_account_factory = SimpleAccountFactory::new(factory_address, provider.clone());
    // calculate the counterfactual address of this account as it would be returned by createAccount()
    let swc_address = simple_account_factory
        .get_address(signer_address, U256::from(SALT))
        .call()
        .await?;

    // Instantiate the UserOperation middleware
    let _uo_middleware = UserOpMiddleware::new(
        provider.clone(),
        GETH_ENTRY_POINT_ADDRESS.to_string().parse::<Address>()?,
        rpc_address,
        chain_id,
    );

    let ep = EntryPoint::new(
        GETH_ENTRY_POINT_ADDRESS.parse::<Address>()?,
        provider.clone(),
    );

    let call = ep.deposit_to(swc_address);

    let mut tx: TypedTransaction = call.tx;
    tx.set_value(parse_ether("10")?);
    println!("tx: {:?}", tx);
    let pending_tx = provider.send_transaction(tx, None).await?;
    println!("pending_tx: {:?}", pending_tx);
    let receipt = pending_tx.await?;
    println!("receipt: {:?}", receipt);

    Ok(())
}
