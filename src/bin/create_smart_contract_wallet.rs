use dotenv::dotenv;
use ethers::{
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::{Http, Middleware, Provider},
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::{transaction::eip2718::TypedTransaction, Address, Bytes, TransactionRequest, U256},
    utils::parse_ether,
};
use ethers_userop::{
    consts::{
        DUMMY_SIGNATURE, GETH_CHAIN_ID, GETH_ENTRY_POINT_ADDRESS, GETH_SIMPLE_ACCOUNT_FACTORY,
        SALT, SEED_PHRASE,
    },
    gen::SimpleAccountFactory,
    types::SimpleAccountExecute,
    UserOpMiddleware,
};
use silius_primitives::{UserOperation, Wallet as UoWallet};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup the environment
    dotenv().ok();
    // let eth_client_address = env::var("HTTP_RPC").unwrap();
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
    let scw_address = simple_account_factory
        .get_address(signer_address, U256::from(SALT))
        .call()
        .await?;

    // Fund the newly created smart contract account
    let tx = TransactionRequest::new()
        .to(scw_address)
        .value(parse_ether("10").unwrap())
        .from(signer_address);
    let _tx = provider.send_transaction(tx, None).await?.await?;

    // Prepare the UserOperation to send
    let nonce = provider.get_transaction_count(scw_address, None).await?;
    let init_calldata = simple_account_factory.create_account(signer_address, U256::from(SALT));
    let tx: TypedTransaction = init_calldata.tx;
    let mut init_code = Vec::new();
    init_code.extend_from_slice(factory_address.as_bytes());
    init_code.extend_from_slice(tx.data().unwrap().to_vec().as_slice());

    // Populate the necessary fields for the UserOperation
    let execution = SimpleAccountExecute::new(Address::zero(), U256::zero(), Bytes::default());
    let (gas_price, priority_fee) = provider.estimate_eip1559_fees(None).await?;
    println!(
        "gas_price: {:?}, priority_fee: {:?}",
        gas_price, priority_fee
    );

    // Create the UserOperation
    let user_op = UserOperation {
        sender: scw_address,
        nonce,
        init_code: Bytes::from(init_code),
        call_data: Bytes::from(execution.encode()),
        call_gas_limit: U256::from(1),
        verification_gas_limit: U256::from(1000000u64),
        pre_verification_gas: U256::from(1u64),
        max_fee_per_gas: U256::from(1),
        max_priority_fee_per_gas: U256::from(1),
        paymaster_and_data: Bytes::new(),
        signature: Bytes::from(DUMMY_SIGNATURE.as_bytes().to_vec()),
    };

    // Create the wallet to sign the UserOperation and sign it
    let uo_wallet = UoWallet::from_phrase(seed.as_str(), &U256::from(chain_id)).unwrap();
    let uo = uo_wallet
        .sign_uo(
            &user_op,
            &GETH_ENTRY_POINT_ADDRESS.to_string().parse::<Address>()?,
            &U256::from(chain_id),
        )
        .await?;

    // Instantiate the UserOperation middleware
    let uo_middleware = UserOpMiddleware::new(
        provider.clone(),
        GETH_ENTRY_POINT_ADDRESS.to_string().parse::<Address>()?,
        rpc_address,
        chain_id,
    );

    // Estimate the gas cost of the UserOperation
    let res = uo_middleware
        .estimate_user_operation_gas(&uo.clone())
        .await?;
    println!("res: {:?}", res);

    // estimated_gas: Response { jsonrpc: "2.0", id: 1, result: EstimateResult { pre_verification_gas: 44572, verification_gas_limit: 340583, call_gas_limit: 21797 } }
    // Send the UserOperation after getting the estimated gas
    let uo = UserOperation {
        pre_verification_gas: res
            .result
            .pre_verification_gas
            .saturating_add(U256::from(1000)),
        // .saturating_add(U256::from(10)),
        // {"jsonrpc":"2.0","error":{"code":-32602,"message":"Pre-verification gas 44582 is lower than calculated pre-verification gas 44656"},"id":1}
        verification_gas_limit: res.result.verification_gas_limit,
        call_gas_limit: res.result.call_gas_limit.saturating_mul(U256::from(2)),
        // call_gas_limit: res.result.call_gas_limit.saturating_add(U256::from(1000)),
        // {"jsonrpc":"2.0","error":{"code":-32602,"message":"Call gas limit 22797 is lower than call gas estimation 23338"},"id":1}
        max_priority_fee_per_gas: priority_fee,
        max_fee_per_gas: gas_price,
        ..user_op
    };

    // Sign the UserOperation
    let signed_uo = uo_wallet
        .sign_uo(
            &uo,
            &GETH_ENTRY_POINT_ADDRESS.to_string().parse::<Address>()?,
            &U256::from(chain_id),
        )
        .await?;

    // Send the UserOperation
    let res = uo_middleware.send_user_operation(&signed_uo).await?;
    println!("res: {:?}", res);

    Ok(())
}
