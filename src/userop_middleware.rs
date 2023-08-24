use crate::{
    consts::{DUMMY_PAYMASTER_AND_DATA, DUMMY_SIGNATURE},
    types::{EstimateResult, Request, Response, UserOpMiddlewareError},
};
use async_trait::async_trait;
use ethers::{
    providers::{Middleware, MiddlewareError},
    types::{Address, Bytes, U256},
};
use hashbrown::HashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use silius_primitives::{
    UserOperation, UserOperationHash, UserOperationPartial, UserOperationReceipt,
};

/// A [ethers-rs](https://docs.rs/ethers/latest/ethers/) middleware that crafts UserOperations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserOpMiddleware<M> {
    /// The inner middleware
    pub inner: M,
    /// The address of the entry point contract
    pub entry_point_address: Address,
    /// The RPC Endpoint to communicate with
    pub rpc_address: String,
    /// The chain id
    pub chain_id: u64,
    /// The client to use for HTTP requests
    #[serde(skip)]
    pub client: Client,
    /// The map of user operation hashes to user operations
    #[serde(skip)]
    pub user_operation_tracker: HashMap<UserOperationHash, UserOperation>,
}

impl<M: Middleware> MiddlewareError for UserOpMiddlewareError<M> {
    type Inner = M::Error;

    fn from_err(src: M::Error) -> Self {
        UserOpMiddlewareError::MiddlewareError(src)
    }

    #[allow(unreachable_patterns)]
    fn as_inner(&self) -> Option<&Self::Inner> {
        match self {
            UserOpMiddlewareError::MiddlewareError(e) => Some(e),
            _ => None,
        }
    }
}

#[async_trait]
impl<M: Middleware> Middleware for UserOpMiddleware<M> {
    type Error = UserOpMiddlewareError<M>;
    type Provider = M::Provider;
    type Inner = M;

    fn inner(&self) -> &M {
        &self.inner
    }
}

impl<M: Middleware> UserOpMiddleware<M> {
    pub fn new(inner: M, entry_point_address: Address, rpc_address: String, chain_id: u64) -> Self {
        let client = reqwest::Client::new();

        Self {
            inner,
            entry_point_address,
            rpc_address,
            chain_id,
            client,
            user_operation_tracker: HashMap::new(),
        }
    }

    pub async fn deploy_smart_contract_wallet(
        &self,
        sender: Address,
        nonce: U256,
        call_data: Bytes,
    ) -> anyhow::Result<()> {
        let uo_partical = self.generate_user_operation_partial(sender, nonce, call_data)?;
        let uo = UserOperation::from(uo_partical);

        let _uo_gas_estimation = self.estimate_user_operation_gas(&uo).await?;

        Ok(())
    }

    /// Generates a user operation partial from the given parameters for gas estimation
    ///
    /// # Arguments
    /// `sender` - The address of the sender
    /// `nonce` - The nonce of the sender
    /// `call_data` - The call data of the transaction
    ///
    /// # Returns
    /// A user operation partial
    pub fn generate_user_operation_partial(
        &self,
        sender: Address,
        nonce: U256,
        call_data: Bytes,
    ) -> anyhow::Result<UserOperationPartial> {
        let user_operation_partial = UserOperationPartial {
            sender: Some(sender),
            nonce: Some(nonce),
            init_code: None,
            call_data: Some(call_data),
            call_gas_limit: None,
            verification_gas_limit: None,
            pre_verification_gas: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            paymaster_and_data: Some(Bytes::from((*DUMMY_PAYMASTER_AND_DATA).as_bytes().to_vec())),
            signature: Some(Bytes::from((*DUMMY_SIGNATURE).as_bytes().to_vec())),
        };

        Ok(user_operation_partial)
    }

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

        let response = self
            .client
            .post(&self.rpc_address)
            .json(&req_body)
            .send()
            .await?;
        let str_response = response.text().await?;
        let res = serde_json::from_str::<Response<EstimateResult>>(&str_response)?;

        Ok(res)
    }

    pub async fn send_user_operation(
        &self,
        uo: &UserOperation,
    ) -> anyhow::Result<Response<UserOperationHash>> {
        let params = vec![json!(uo), json!(self.entry_point_address)];

        let req_body = Request {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendUserOperation".to_string(),
            params: params.clone(),
            id: 1,
        };

        let response = self
            .client
            .post(&self.rpc_address)
            .json(&req_body)
            .send()
            .await?;
        let str_response = response.text().await?;
        println!("{}", str_response);
        let res = serde_json::from_str::<Response<UserOperationHash>>(&str_response)?;
        println!("{:?}", res);

        Ok(res)
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
        let params = vec![json!(user_operation_hash)];

        let response = self
            .client
            .post(&self.rpc_address)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_getUserOperationReceipt",
                "params": params,
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
        let params = vec![json!(user_operation_hash)];

        let response = self
            .client
            .post(&self.rpc_address)
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "eth_getUserOperationByHash",
                "params": params,
                "id": 1,
            }))
            .send()
            .await?
            .json::<Response<String>>()
            .await?;

        Ok(response.result)
    }
}
