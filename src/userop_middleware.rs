use crate::{
    consts::DUMMY_SIGNATURE,
    errors::{UserOpBuilderError, UserOpMiddlewareError},
    gen::SimpleAccount,
    traits::SmartWalletAccount,
    types::{ErrorResponse, EstimateResult, Request, Response, WalletMap},
    uo_builder::UserOperationBuilder,
};
use async_trait::async_trait;
use ethers::{
    contract::abigen,
    middleware::{MiddlewareBuilder, SignerMiddleware},
    providers::{Middleware, MiddlewareError},
    signers::Signer,
    types::{transaction::eip2718::TypedTransaction, Address, Bytes, TransactionRequest, U256},
    utils::parse_ether,
};
use hashbrown::HashMap;
use parking_lot::Mutex;
use rand::Rng;
use regex::Regex;
use serde_json::json;
use silius_primitives::{UserOperation, UserOperationHash, UserOperationReceipt, Wallet};
use std::fmt;
use std::sync::Arc;

abigen!(EntryPoint, "src/abi/EntryPoint.json",);
/// A [ethers-rs](https://docs.rs/ethers/latest/ethers/) middleware that crafts UserOperations
#[derive(Clone)]
pub struct UserOpMiddleware<M> {
    /// The inner middleware
    pub inner: M,
    /// The address of the entry point contract
    pub entry_point_address: Address,
    /// The bundler's RPC Endpoint to communicate with
    pub rpc_address: String,
    /// The chain id
    pub chain_id: u64,
    /// Wallet used to sign a UserOperation
    #[doc(hidden)]
    pub wallet: Wallet,
    /// A hashmap to store deployed smart contract wallet addresses
    pub wallet_map: WalletMap,
}

impl<M: Middleware + 'static + fmt::Debug + Clone> fmt::Debug for UserOpMiddleware<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserOpMiddleware")
            .field("inner", &self.inner)
            .field("entry_point_address", &self.entry_point_address)
            .field("rpc_address", &self.rpc_address)
            .field("chain_id", &self.chain_id)
            .finish()
    }
}
impl<M: Middleware + 'static + fmt::Debug + Clone> MiddlewareError for UserOpMiddlewareError<M> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> Self {
        UserOpMiddlewareError::MiddlewareError(src)
    }

    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            UserOpMiddlewareError::MiddlewareError(e) => Some(e),
            _ => None,
        }
    }
}

#[async_trait]
impl<M: Middleware + 'static + fmt::Debug + Clone> Middleware for UserOpMiddleware<M> {
    type Error = UserOpMiddlewareError<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }
}
impl<M: Middleware + 'static + fmt::Debug + Clone> UserOpMiddleware<M> {
    pub fn new(
        inner: M,
        entry_point_address: Address,
        rpc_address: impl Into<String>,
        wallet: Wallet,
    ) -> Self {
        let chain_id = wallet.signer.chain_id();

        // Add a default key-value pair to the wallet_map
        let wallet_account = Box::new(SimpleAccount::new(Address::default(), inner.clone().into()));
        let wallet_contract: Box<dyn SmartWalletAccount> = wallet_account;
        let mut wallet_map = HashMap::new();
        wallet_map.insert(Address::default(), Arc::new(Mutex::new(wallet_contract)));

        Self {
            inner,
            entry_point_address,
            rpc_address: rpc_address.into(),
            chain_id,
            wallet,
            wallet_map,
        }
    }

    #[allow(dead_code)]
    fn entry_point_address(&self) -> &Address {
        &self.entry_point_address
    }

    #[allow(dead_code)]
    fn rpc_address(&self) -> &String {
        &self.rpc_address
    }

    #[allow(dead_code)]
    fn wallet(&self) -> &Wallet {
        &self.wallet
    }

    #[allow(dead_code)]
    fn wallet_map(&self) -> &WalletMap {
        &self.wallet_map
    }

    ////////////////////////////////////////////////////// Bundler RPC methods //////////////////////////////////////////////////////

    pub async fn estimate_user_operation_gas(
        &self,
        user_operation: &UserOperation,
    ) -> anyhow::Result<Response<EstimateResult>> {
        let params = vec![json!(user_operation), json!(self.entry_point_address)];

        let req_body = Request {
            jsonrpc: "2.0".to_string(),
            method: "eth_estimateUserOperationGas".to_string(),
            params: params.clone(),
            id: 1,
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&self.rpc_address)
            .json(&req_body)
            .send()
            .await?;

        Self::handle_response(response).await
    }

    pub async fn send_user_operation(
        &self,
        uo: &UserOperation,
    ) -> anyhow::Result<Response<UserOperationHash>> {
        let req_body = Request {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendUserOperation".to_string(),
            params: vec![json!(uo), json!(self.entry_point_address)],
            id: 1,
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&self.rpc_address)
            .json(&req_body)
            .send()
            .await?;

        Self::handle_response(response).await
    }

    pub fn supported_entry_point(&self) -> Address {
        self.entry_point_address
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    pub async fn get_user_operation_receipt(
        &self,
        user_operation_hash: &UserOperationHash,
    ) -> anyhow::Result<UserOperationReceipt> {
        let client = reqwest::Client::new();
        let response = client
            .post(&self.rpc_address)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_getUserOperationReceipt",
                "params": vec![json!(user_operation_hash)],
                "id": 1,
            }))
            .send()
            .await?
            .json::<Response<UserOperationReceipt>>()
            .await?;

        Ok(response.result)
    }

    pub async fn get_user_operation_by_hash(
        &self,
        user_operation_hash: &UserOperationHash,
    ) -> anyhow::Result<String> {
        let client = reqwest::Client::new();
        let response = client
            .post(&self.rpc_address)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_getUserOperationByHash",
                "params": vec![json!(user_operation_hash)],
                "id": 1,
            }))
            .send()
            .await?
            .json::<Response<String>>()
            .await?;

        Ok(response.result)
    }

    /// Helper function to handle the response from the bundler
    ///
    /// # Arguments
    /// * `response` - The response from the bundler
    ///
    /// # Returns
    /// * `Response<R>` - The success response if no error
    async fn handle_response<R>(response: reqwest::Response) -> anyhow::Result<Response<R>>
    where
        R: std::fmt::Debug + serde::de::DeserializeOwned,
    {
        let str_response = response.text().await?;
        let parsed_response: anyhow::Result<Response<R>> =
            serde_json::from_str(&str_response).map_err(anyhow::Error::from);

        match parsed_response {
            Ok(success_response) => {
                log::info!("Success {:?}", success_response);
                Ok(success_response)
            }
            Err(_) => {
                let error_response: ErrorResponse = serde_json::from_str(&str_response)?;
                log::warn!("Error: {:?}", error_response);
                let error_message = &error_response.error.message;

                if let Some(captures) =
                    Regex::new(r"Call gas limit (\d+) is lower than call gas estimation (\d+)")
                        .unwrap()
                        .captures(error_message)
                {
                    let limit: u64 = captures[1].parse().unwrap();
                    let estimation: u64 = captures[2].parse().unwrap();
                    return Err(anyhow::anyhow!(
                        UserOpMiddlewareError::<M>::CallGasLimitError(limit, estimation,)
                    ));
                }

                if let Some(captures) = Regex::new(r"Pre-verification gas (\d+) is lower than calculated pre-verification gas (\d+)")
                        .unwrap()
                        .captures(error_message)
                {
                    let pre_verification_gas: u64 = captures[1].parse().unwrap();
                    let calculated_gas: u64 = captures[2].parse().unwrap();
                    return Err(anyhow::anyhow!(
                        UserOpMiddlewareError::<M>::PreVerificationGasError(pre_verification_gas, calculated_gas)
                    ));
                }

                if error_message.contains("AA40 over verificationGasLimit") {
                    return Err(anyhow::anyhow!(
                        UserOpMiddlewareError::<M>::VerificationGasLimitError
                    ));
                }

                Err(anyhow::anyhow!(UserOpMiddlewareError::<M>::UnknownError))
            }
        }
    }
    ////////////////////////////////////////////////////// UserOperation APIs //////////////////////////////////////////////////////

    /// Call arbitrary function from the smart contract wallet
    ///
    /// # Arguments
    /// * `wallet_name` - the name of the wallet. i.e. SoulWallet
    /// * `scw_wallet_address` - the address of the deployed smart contract wallet
    /// * `tx` - a [TypedTransaction](ethers::types::transaction::eip2718::TypedTransaction)
    ///
    /// # Returns
    /// * `Response<UserOperationHash>` - The UserOperationHash of the UserOperation sent
    pub async fn call(
        &self,
        wallet_name: impl Into<String>,
        scw_wallet_address: Address,
        tx: TypedTransaction,
    ) -> anyhow::Result<Response<UserOperationHash>> {
        let (call_data, dest, value) = self.uo_calldata_from_tx(tx).await?;
        let provider = Arc::new(self.inner.clone());
        let sender = self.wallet.signer.address();
        let mut uo_builder = UserOperationBuilder::new(
            sender,
            wallet_name,
            Some(scw_wallet_address),
            provider.clone(),
            None,
        )?;

        let scw_code = provider.get_code(scw_wallet_address, None).await?;
        if scw_code.0.is_empty() {
            return Err(anyhow::anyhow!(
                UserOpMiddlewareError::<M>::UserOpBuilderError(
                    UserOpBuilderError::<M>::SmartContractWalletHasNotBeenDeployed
                )
            ));
        };

        let nonce = EntryPoint::new(self.entry_point_address, self.inner.clone().into())
            .get_nonce(scw_wallet_address, U256::zero())
            .await?;

        let execution_calldata = uo_builder.wallet_contract().execute(dest, value, call_data);

        let (gas_price, priority_fee) = self.inner.estimate_eip1559_fees(None).await?;

        let uo = uo_builder
            .set_uo_sender(scw_wallet_address)
            .set_uo_nonce(nonce)
            .set_uo_init_code(Bytes::default())
            .set_uo_calldata(execution_calldata.into())
            .set_uo_call_gas_limit(1u64.into())
            .set_uo_pre_verification_gas(1u64.into())
            .set_uo_verification_gas_limit(1000000u64.into())
            .set_uo_max_fee_per_gas(1.into())
            .set_uo_max_priority_fee_per_gas(priority_fee)
            .set_uo_paymaster_and_data(Bytes::new())
            .set_uo_signature(DUMMY_SIGNATURE.as_bytes().to_vec().into())
            .build_uo()?;

        let signed_uo = self.sign_uo(uo.clone()).await?;
        let estimate_result = self.estimate_user_operation_gas(&signed_uo).await?;

        let mut pre_verification_gas = estimate_result.result.pre_verification_gas;
        let mut call_gas_limit = estimate_result.result.call_gas_limit;
        let mut verification_gas_limit = estimate_result
            .result
            .verification_gas_limit
            .saturating_add(U256::from(10000));
        let mut uo_hash = None;

        while uo_builder.uo_hash().is_none() {
            uo_builder
                .set_uo_pre_verification_gas(pre_verification_gas)
                .set_uo_call_gas_limit(call_gas_limit)
                .set_uo_verification_gas_limit(verification_gas_limit)
                .set_uo_max_fee_per_gas(gas_price)
                .set_uo_max_priority_fee_per_gas(priority_fee);

            let uo = uo_builder.build_uo()?;
            let signed_uo = self.sign_uo(uo.clone()).await?;

            match self.send_user_operation(&signed_uo).await {
                Ok(success_response) => {
                    uo_hash = Some(success_response);
                    let _ = uo_builder.set_uo_hash(uo_hash.as_ref().unwrap().result);
                }
                Err(err) => {
                    if let Some(custom_err) = err.downcast_ref::<UserOpMiddlewareError<M>>() {
                        match custom_err {
                            UserOpMiddlewareError::CallGasLimitError(_limit, estimation) => {
                                call_gas_limit = U256::from(*estimation); // Set the call_gas_limit to the estimated value
                                log::warn!("Call gas limit is not enough. Retry with call_gas_limit increased to {}", &estimation);
                            }
                            UserOpMiddlewareError::PreVerificationGasError(
                                _pre_verification,
                                calculated,
                            ) => {
                                pre_verification_gas = U256::from(*calculated); // Set the pre_verification_gas to the calculated value
                                log::warn!("Pre-verification gas is not enough. Retry with pre_verification_gas increased to {}", &calculated);
                            }
                            UserOpMiddlewareError::VerificationGasLimitError => {
                                verification_gas_limit += U256::from(10000); // Increase the limit by an arbitrary amount
                                log::warn!("Verification gas limit is not enough. Retry with verification_gas_limit increased to {}", &verification_gas_limit);
                            }
                            _ => {
                                return Err(anyhow::anyhow!(
                                    UserOpMiddlewareError::<M>::UnknownError
                                ));
                            }
                        }
                    }
                }
            }
        }
        let uo_hash = uo_hash.unwrap();

        Ok(uo_hash)
    }

    /// API to convert
    /// a [TypedTransaction](ethers::types::transaction::eip2718::TypedTransaction)
    /// into the `calldata` field of a UserOperation
    ///
    /// # Arguments
    /// * `tx` - The TypedTransaction
    ///
    /// # Returns
    /// * `Bytes` - The `calldata` field of the UserOperation
    pub async fn uo_calldata_from_tx(
        &self,
        tx: TypedTransaction,
    ) -> anyhow::Result<(Bytes, Address, U256)> {
        let calldata: Bytes;
        let dest: Address;
        let value: U256;
        match tx {
            TypedTransaction::Eip1559(tx_req) => {
                calldata = tx_req.data.expect("No `data` in transaction request");
                dest = *tx_req
                    .to
                    .expect("No `to` address in transaction request")
                    .as_address()
                    .unwrap();
                value = tx_req.value.expect("No `value` in transaction request");
            }
            TypedTransaction::Legacy(tx_req) => {
                calldata = tx_req.data.expect("No `data` in transaction request");
                dest = *tx_req
                    .to
                    .expect("No `to` address in transaction request")
                    .as_address()
                    .unwrap();
                value = tx_req.value.expect("No `value` in transaction request");
            }
            TypedTransaction::Eip2930(tx_req) => {
                calldata = tx_req.tx.data.expect("No `data` in transaction request");
                dest = *tx_req
                    .tx
                    .to
                    .expect("No `to` address in transaction request")
                    .as_address()
                    .unwrap();
                value = tx_req.tx.value.expect("No `value` in transaction request");
            }
        };

        Ok((calldata, dest, value))
    }

    /// API to Build a [UserOperationBuilder](crate::uo_builder::UserOperationBuilder) with a random salt
    ///
    /// # Arguments
    /// * `wallet_name` - The name of the wallet
    ///
    /// # Returns
    /// * `UserOperationBuilder<M>` - The user operation builder
    pub fn build_random_uo_builder(
        &self,
        wallet_name: String,
    ) -> anyhow::Result<UserOperationBuilder<M>> {
        let sender_address = self.wallet.signer.address();
        let salt = rand::thread_rng().gen::<u64>();

        UserOperationBuilder::new(
            sender_address,
            wallet_name,
            None,
            self.inner.clone().into(),
            Some(salt),
        )
    }

    /// API to generates a random counter factual address
    ///
    /// # Arguments
    /// * `wallet_name` - The name of the wallet
    ///
    /// # Returns
    /// * `Address` - The counter factual address
    pub async fn build_random_address(&self, wallet_name: String) -> anyhow::Result<Address> {
        let provider = Arc::new(self.inner.clone());
        let sender = self.wallet.signer.address();
        let salt = rand::thread_rng().gen::<u64>();
        let mut uo_builder =
            UserOperationBuilder::new(sender, wallet_name, None, provider.clone(), Some(salt))?;

        // Generate the smart contract wallet address
        let scw_address = uo_builder.set_scw_address().await?;
        Ok(scw_address)
    }

    /// API to deploys a smart contract wallet with a random salt
    ///
    /// # Arguments
    /// * `wallet_name` - The name of the smart contract wallet i.e. SoulWallet
    /// * `pre_fund` - The amount of Ether to pre-fund the smart contract wallet
    /// # Returns
    /// * `Response<UserOperationHash>` - The UserOperationHash of the UserOperation sent
    /// * `Address` - The address of the deployed smart contract wallet
    pub async fn deploy_random_scw(
        &mut self,
        wallet_name: String,
        pre_fund: u64,
    ) -> anyhow::Result<(Response<UserOperationHash>, Address)> {
        let (uo_hash, scw_address) = self
            .deploy_scw(wallet_name, rand::thread_rng().gen::<u64>(), pre_fund)
            .await?;
        Ok((uo_hash, scw_address))
    }

    /// Send eth to an address
    ///
    /// # Arguments
    /// * `scw_wallet_address` - The address of the smart contract wallet
    /// * `wallet_name` - The name of the wallet
    /// * `to` - The address to send eth to
    /// * `amount` - The amount of eth to send
    ///
    /// # Returns
    /// *`Response<UserOperationHash>` - The UserOperationHash of the UserOperation sent
    pub async fn send_eth(
        &mut self,
        scw_wallet_address: Address,
        wallet_name: impl Into<String>,
        to: Address,
        amount: u64,
    ) -> anyhow::Result<Response<UserOperationHash>> {
        let provider = Arc::new(self.inner.clone());
        let sender = self.wallet.signer.address();
        let mut uo_builder = UserOperationBuilder::new(
            sender,
            wallet_name,
            Some(scw_wallet_address),
            provider.clone(),
            None,
        )?;

        let scw_code = provider.get_code(scw_wallet_address, None).await?;
        if scw_code.0.is_empty() {
            return Err(anyhow::anyhow!(
                UserOpMiddlewareError::<M>::UserOpBuilderError(
                    UserOpBuilderError::<M>::SmartContractWalletHasNotBeenDeployed
                )
            ));
        };

        let nonce = EntryPoint::new(self.entry_point_address, self.inner.clone().into())
            .get_nonce(scw_wallet_address, U256::zero())
            .await?;

        let execution_calldata = uo_builder.wallet_contract().execute(
            Address::from(to),
            U256::from(amount),
            Bytes::default(),
        );

        let (gas_price, priority_fee) = self.inner.estimate_eip1559_fees(None).await?;

        let uo = uo_builder
            .set_uo_sender(scw_wallet_address)
            .set_uo_nonce(nonce)
            .set_uo_init_code(Bytes::default())
            .set_uo_calldata(execution_calldata.into())
            .set_uo_call_gas_limit(1u64.into())
            .set_uo_pre_verification_gas(1u64.into())
            .set_uo_verification_gas_limit(1000000u64.into())
            .set_uo_max_fee_per_gas(1.into())
            .set_uo_max_priority_fee_per_gas(priority_fee)
            .set_uo_paymaster_and_data(Bytes::new())
            .set_uo_signature(DUMMY_SIGNATURE.as_bytes().to_vec().into())
            .build_uo()?;

        let signed_uo = self.sign_uo(uo.clone()).await?;
        let estimate_result = self.estimate_user_operation_gas(&signed_uo).await?;

        let mut pre_verification_gas = estimate_result.result.pre_verification_gas;
        let mut call_gas_limit = estimate_result.result.call_gas_limit;
        let mut verification_gas_limit = estimate_result
            .result
            .verification_gas_limit
            .saturating_add(U256::from(10000));
        let mut uo_hash = None;

        while uo_builder.uo_hash().is_none() {
            uo_builder
                .set_uo_pre_verification_gas(pre_verification_gas)
                .set_uo_call_gas_limit(call_gas_limit)
                .set_uo_verification_gas_limit(verification_gas_limit)
                .set_uo_max_fee_per_gas(gas_price)
                .set_uo_max_priority_fee_per_gas(priority_fee);

            let uo = uo_builder.build_uo()?;
            let signed_uo = self.sign_uo(uo.clone()).await?;

            match self.send_user_operation(&signed_uo).await {
                Ok(success_response) => {
                    uo_hash = Some(success_response);
                    let _ = uo_builder.set_uo_hash(uo_hash.as_ref().unwrap().result);
                }
                Err(err) => {
                    if let Some(custom_err) = err.downcast_ref::<UserOpMiddlewareError<M>>() {
                        match custom_err {
                            UserOpMiddlewareError::CallGasLimitError(_limit, estimation) => {
                                call_gas_limit = U256::from(*estimation); // Set the call_gas_limit to the estimated value
                                log::warn!("Call gas limit is not enough. Retry with call_gas_limit increased to {}", &estimation);
                            }
                            UserOpMiddlewareError::PreVerificationGasError(
                                _pre_verification,
                                calculated,
                            ) => {
                                pre_verification_gas = U256::from(*calculated); // Set the pre_verification_gas to the calculated value
                                log::warn!("Pre-verification gas is not enough. Retry with pre_verification_gas increased to {}", &calculated);
                            }
                            UserOpMiddlewareError::VerificationGasLimitError => {
                                verification_gas_limit += U256::from(1000); // Increase the limit by an arbitrary amount
                                log::warn!("Verification gas limit is not enough. Retry with verification_gas_limit increased to {}", &verification_gas_limit);
                            }
                            _ => {
                                return Err(anyhow::anyhow!(
                                    UserOpMiddlewareError::<M>::UnknownError
                                ));
                            }
                        }
                    }
                }
            }
        }
        let uo_hash = uo_hash.unwrap();

        Ok(uo_hash)
    }

    /// API to deploy a smart contract wallet
    /// Note: for now, the `call_gas_limit` and `pre_verification_gas` are arbitrarily set
    /// after gas estimation
    ///
    /// # Arguments
    /// * `wallet_name` - The name of the smart contract wallet i.e. SoulWallet
    /// * `pre_fund` - The amount of Ether to pre-fund the smart contract wallet
    /// * `salt` - The salt used to generate a smart contract wallet address
    ///
    /// # Returns
    /// * `Response<UserOperationHash>` - The UserOperationHash of the UserOperation
    /// * `Address` - The address of the deployed smart contract wallet
    pub async fn deploy_scw(
        &mut self,
        wallet_name: String,
        pre_fund: u64,
        salt: u64,
    ) -> anyhow::Result<(Response<UserOperationHash>, Address)> {
        let mut uo_builder = self.create_scw_deployment_uo(wallet_name, salt).await?;

        let scw_address = uo_builder.scw_address().unwrap();
        let signer_address = uo_builder.signer_address();

        let client = SignerMiddleware::new(
            self.inner.clone(),
            self.wallet.signer.clone().with_chain_id(self.chain_id),
        )
        .nonce_manager(signer_address);
        let provider = Arc::new(client);

        let tx = TransactionRequest::new()
            .to(scw_address)
            .value(parse_ether(pre_fund).unwrap())
            .from(signer_address);
        let _tx = provider.send_transaction(tx, None).await?.await?;

        let uo = uo_builder.build_uo()?;
        let signed_uo = self.sign_uo(uo.clone()).await?;

        let estimate_result = self.estimate_user_operation_gas(&signed_uo).await?;

        let (gas_price, priority_fee) = self.inner.estimate_eip1559_fees(None).await?;

        // TODO: add a method to increase the call gas limit and pre verification gas incrementally
        uo_builder
            .set_uo_pre_verification_gas(
                estimate_result
                    .result
                    .pre_verification_gas
                    .saturating_add(U256::from(1000)),
            )
            .set_uo_call_gas_limit(
                estimate_result
                    .result
                    .call_gas_limit
                    .saturating_mul(U256::from(2)),
            )
            .set_uo_verification_gas_limit(estimate_result.result.verification_gas_limit)
            .set_uo_max_fee_per_gas(gas_price)
            .set_uo_max_priority_fee_per_gas(priority_fee);

        let uo = uo_builder.build_uo()?;
        let signed_uo = self.sign_uo(uo.clone()).await?;

        let uo_hash = self.send_user_operation(&signed_uo).await?;
        let _ = uo_builder.set_uo_hash(uo_hash.result);

        let wallet_contract = uo_builder.wallet_contract();
        self.wallet_map
            .insert(scw_address, Arc::new(Mutex::new(wallet_contract)));
        Ok((uo_hash, scw_address))
    }

    /// Sign the UserOperation
    ///
    /// # Arguments
    /// * `uo` - The UserOperation to sign
    /// # Returns
    /// * `UserOperation` - The signed UserOperation
    pub async fn sign_uo(&self, uo: UserOperation) -> anyhow::Result<UserOperation> {
        self.wallet
            .sign_uo(&uo, &self.entry_point_address, &U256::from(self.chain_id))
            .await
    }

    /// API to create a UserOperation for smart contract deployment
    /// Note: if the generated counter-factual has been previously deployed, the function will
    /// throw a [SmartContractWalletHasBeenDeployed](SmartContractWalletHasBeenDeployed) error
    ///
    /// # Arguments
    /// * `wallet_name` - The name of the smart contract wallet i.e. SoulWallet
    /// * `factory_address` - The address of the factory contract
    /// * `salt` - The salt used to generate a smart contract wallet address
    ///
    /// # Returns
    /// * `UserOperation` - The UserOperation created
    pub async fn create_scw_deployment_uo(
        &self,
        wallet_name: String,
        salt: u64,
    ) -> anyhow::Result<UserOperationBuilder<M>> {
        let provider = Arc::new(self.inner.clone());
        let sender = self.wallet.signer.address();
        let mut uo_builder =
            UserOperationBuilder::new(sender, wallet_name, None, provider.clone(), Some(salt))?;

        // Generate the smart contract wallet address
        let scw_address = uo_builder.set_scw_address().await?;
        let signer_address = uo_builder.signer_address();

        // Check balance of the generated scw_address. If it's non-zero, it means it has been
        // previously deployed
        let scw_code = provider.get_code(scw_address, None).await?;
        if !scw_code.0.is_empty() {
            return Err(anyhow::anyhow!(
                UserOpMiddlewareError::<M>::UserOpBuilderError(
                    UserOpBuilderError::<M>::SmartContractWalletHasBeenDeployed
                )
            ));
        };

        // Prepare the UserOperation to send
        let nonce = provider.get_transaction_count(scw_address, None).await?;
        let init_calldata = uo_builder
            .factory_contract()
            .create_account(signer_address, salt.into());
        let tx: TypedTransaction = init_calldata.tx;
        let mut init_code = Vec::new();
        init_code.extend_from_slice(uo_builder.factory_contract_address().as_bytes());
        init_code.extend_from_slice(tx.data().unwrap().to_vec().as_slice());

        // Empty calldata for contract deployment
        let execution_calldata =
            uo_builder
                .wallet_contract()
                .execute(Address::zero(), U256::zero(), Bytes::default());

        // Create the UserOperation
        uo_builder
            .set_uo_sender(scw_address)
            .set_uo_nonce(nonce)
            .set_uo_init_code(init_code.clone().into())
            .set_uo_calldata(execution_calldata.clone().into())
            .set_uo_call_gas_limit(1.into())
            .set_uo_pre_verification_gas(1.into())
            .set_uo_verification_gas_limit(1000000u64.into())
            .set_uo_max_fee_per_gas(1.into())
            .set_uo_max_priority_fee_per_gas(1.into())
            .set_uo_paymaster_and_data(Bytes::new())
            .set_uo_signature(DUMMY_SIGNATURE.as_bytes().to_vec().into());

        Ok(uo_builder)
    }
}
