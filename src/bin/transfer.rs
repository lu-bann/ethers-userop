use ethers::{
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::{Http, Middleware, Provider},
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::{transaction::eip2718::TypedTransaction, Address, Bytes, TransactionRequest, U256},
    utils::parse_ether,
};
use ethers_userop::{
    consts::{DUMMY_SIGNATURE, GETH_CHAIN_ID, GETH_SIMPLE_ACCOUNT_FACTORY, SALT, SEED_PHRASE},
    gen::{SimpleAccountExecute, SimpleAccountFactory},
};
use silius_primitives::UserOperation;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let eth_client_address = "http://localhost:8545".to_string();
    let seed = SEED_PHRASE.to_string();
    let provider = Arc::new(Provider::<Http>::try_from(eth_client_address.to_string())?);
    let _rpc_address = format!("http://{}", "127.0.0.1:3000");
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

    // Fund the newly created smart contract account
    let tx = TransactionRequest::new()
        .to(swc_address)
        .value(parse_ether("10").unwrap())
        .from(signer_address);
    let _tx = provider.send_transaction(tx, None).await?.await?;

    // Prepare the UserOperation to send
    let nonce = provider.get_transaction_count(swc_address, None).await?;
    let init_calldata = simple_account_factory.create_account(signer_address, U256::from(SALT));
    let tx: TypedTransaction = init_calldata.tx;
    let mut init_code = Vec::new();
    init_code.extend_from_slice(factory_address.as_bytes());
    init_code.extend_from_slice(tx.data().unwrap().to_vec().as_slice());

    // Populate the necessary fields for the UserOperation
    let execution = SimpleAccountExecute::new(swc_address.clone(), U256::from(1), Bytes::default());
    let (gas_price, priority_fee) = provider.estimate_eip1559_fees(None).await?;
    println!(
        "gas_price: {:?}, priority_fee: {:?}",
        gas_price, priority_fee
    );

    // Create the UserOperation
    let _user_op = UserOperation {
        sender: swc_address,
        nonce,
        init_code: Bytes::default(),
        call_data: Bytes::from(execution.encode()),
        call_gas_limit: U256::from(1),
        verification_gas_limit: U256::from(1000000u64),
        pre_verification_gas: U256::from(1u64),
        max_fee_per_gas: U256::from(1),
        max_priority_fee_per_gas: U256::from(1),
        paymaster_and_data: Bytes::new(),
        signature: Bytes::from(DUMMY_SIGNATURE.as_bytes().to_vec()),
    };

    Ok(())
}
