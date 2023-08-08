use crate::types::UserOpMiddleware;
use async_trait::async_trait;
use silius_bundler_primitives::{
	UserOperationGasEstimate, UserOperationPartial, UserOperationHash,
	UserOperationReceipt,
};
use ethers::{
	providers::{Middleware, Provider},
	types::{Address, Block, Bytes, U256},
};
use reqwest::Client;
use hashbrown::HashMap;

const DUMMY_PAYMASTER_AND_DATA: &str = "0xC03Aac639Bb21233e0139381970328dB8bcEeB67fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000007aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1c";
const DUMMY_SIGNATURE: &str = "0xfffffffffffffffffffffffffffffff0000000000000000000000000000000007aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1c";

#[async_trait]
impl<M: Middleware> Middleware for UserOpMiddleware<M> {
	type Error = M::Error;
	type Provider = M::Provider;
	type Inner = M;

	fn inner(&self) -> &M {
		self.inner
	}

	async fn estimate_gas(
		&self,
		tx: &ethers::types::TransactionRequest,
	) -> Result<ethers::types::U256, Self::Error> {
		let sender = tx.from.unwrap_or_else(|| self.inner.get_signer().unwrap());
		let nonce = self
			.inner
			.get_transaction_count(sender, Some(Block::Pending.into()))
			.await?;
		let call_data = tx.data.clone().unwrap_or_default();

		let user_operation_partial = self
			.generate_user_operation_partial(sender, nonce, call_data)
			.unwrap();

		let user_operation_gas_estimation = self
			.estimate_user_operation_gas(&user_operation_partial)
			.await?;

		Ok(user_operation_gas_estimation.total_gas)

	}

	async fn send_transaction(
		&self,
		tx: &ethers::types::TransactionRequest,
	) -> Result<ethers::types::TransactionResponse<Self::Provider>, Self::Error> {
		todo!()
	}
}

impl<M: Middleware> UserOpMiddleware<M> {

	pub fn new(
		inner: M,
		entry_point_address: Address,
		rpc_address: String,
		chain_id: u64,
	) -> Self {

		let mut client = reqwest::Client::new();

		Self {
			inner,
			entry_point_address,
			rpc_address,
			chain_id,
			client,
			user_operation_tracker: HashMap::new(),
		}
	}

	pub fn generate_user_operation_partial(
		&mut self,
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
			paymaster_and_data: Some(Bytes::from(DUMMY_PAYMASTER_AND_DATA)),
			signature: Some(Bytes::from(DUMMY_SIGNATURE)),
		};

		Ok(user_operation_partial)
	}

	pub async fn estimate_user_operation_gas(
		&self,
		user_operation_partial: &UserOperationPartial,
	) -> anyhow::Result<UserOperationGasEstimation> {

		let mut params = Vec::new();
		params.push(json!(user_operation_partial));

		let response = self
			.client
			.post(&self.rpc_address)
			.json(&json!({
				"jsonrpc": "2.0",
				"method": "eth_estimateUserOperationGas",
				"params": params,
				"id": 1,
			}))
			.send()
			.await?
			.json::<Response<UserOperationGasEstimation>>()?;

		Ok(response.result)
	}

	pub async fn send_user_operation(
		&self,
		uo: &UserOperation,
	) -> anyhow::Result<UserOperationHash> {

		let mut params = Vec::new();
		params.push(json!(uo));

		let response = self
			.client
			.post(&self.rpc_address)
			.json(&json!({
				"jsonrpc": "2.0",
				"method": "eth_sendUserOperation",
				"params": params,
				"id": 1,
			}))
			.send()
			.await?
			.json::<Response<UserOperationHash>>()?;

		match response.result {
			Some(user_operation_hash) => Ok(
				{

					self.user_operation_tracker
						.insert(user_operation_hash, uo.clone());

					Ok(
						user_operation_hash
					)
				}
			),
			None => Err(anyhow::anyhow!("No user operation hash returned")),
		}

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

		let mut params = Vec::new();
		params.push(json!(user_operation_hash));

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
			.json::<Response<UserOperationReceipt>>()?;

		Ok(response.result)
	}	

	pub async fn get_user_operation_by_hash(
		&self,
		user_operation_hash: &UserOperationHash,
	) -> anyhow::Result<String> {

		let mut params = Vec::new();
		params.push(json!(user_operation_hash));

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
			.json::<Response<String>>()?;

		Ok(response.result)
	}


	
} 