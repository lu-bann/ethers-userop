use ethers::{
    providers::{Middleware, Provider},
    types::{Address, Block, U256},
};
use reqwest::Client;
use silius_bundler_primitives::{
	UserOperation, UserOperationHash
};
use hashbrown::HashMap;

/// A [ethers-rs](https://docs.rs/ethers/latest/ethers/) middleware that crafts UserOperations
#[derive(Clone, Debug)]
pub struct UserOpMiddleware<M> {
	/// The inner middleware
	pub(crate) inner: M,
	/// The address of the entry point contract
	pub(crate) entry_point_address: Address,
	/// The RPC Endpoint to communicate with
	pub(crate) rpc_address: String,
	/// The chain id
	pub (crate) chain_id: u64,
	/// The client to use for HTTP requests
	pub(crate) client: Client,
	/// The map of user operation hashes to user operations
	pub (crate) user_operation_tracker: HashMap<UserOperationHash, UserOperation>,
}

// Error thrown when the UserOpMiddleware interacts with the bundlers
// #[derive(Debug, Clone)]
// pub enum UserOpMiddlewareError<M: Middleware> {
// }