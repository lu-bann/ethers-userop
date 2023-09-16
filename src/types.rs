use crate::consts::{GETH_SIMPLE_ACCOUNT_FACTORY, SIMPLE_ACCOUNT_FACTORY};
use crate::traits::SmartWalletAccount;
use ethers::{
    prelude::{NonceManagerMiddleware, SignerMiddleware},
    signers::LocalWallet,
    types::{Address, U256},
};
use hashbrown::HashMap;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

/// Nonce manager middleware type alias
pub type SignerType<M> = NonceManagerMiddleware<SignerMiddleware<Arc<M>, LocalWallet>>;

/// A map of wallet addresses to their respective [SmartWalletAccount](SmartWalletAccount) instances
pub type WalletMap = HashMap<Address, Arc<Mutex<Box<dyn SmartWalletAccount>>>>;

#[derive(Debug, Serialize)]
pub struct Request<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: T,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EstimateResult {
    pub pre_verification_gas: U256,
    pub verification_gas_limit: U256,
    pub call_gas_limit: U256,
}

#[derive(Debug, Deserialize)]
pub struct Response<R> {
    pub jsonrpc: String,
    pub id: u64,
    pub result: R,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorResponse {
    pub(crate) jsonrpc: String,
    pub(crate) id: u64,
    pub(crate) error: JsonRpcError,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

pub struct DeployedContract<C> {
    contract: C,
    pub address: Address,
}

impl<C> DeployedContract<C> {
    pub fn new(contract: C, addr: Address) -> Self {
        Self {
            contract,
            address: addr,
        }
    }

    pub fn contract(&self) -> &C {
        &self.contract
    }
}

/// A collection of supported wallets
pub enum WalletRegistry {
    SimpleAccount,
}

impl WalletRegistry {
    pub fn from_str(s: &str) -> anyhow::Result<WalletRegistry> {
        match s {
            "simple-account" => Ok(WalletRegistry::SimpleAccount),
            "simple-account-test" => Ok(WalletRegistry::SimpleAccount),
            _ => Err(anyhow::anyhow!("{} wallet currently not supported", s)),
        }
    }
}

/// A collection of supported wallet factories
pub enum WalletFactoryRegistry {
    SimpleAccountFactory(Address),
}

impl WalletFactoryRegistry {
    pub fn from_str(s: &str) -> anyhow::Result<WalletFactoryRegistry> {
        match s {
            "simple-account" => Ok(WalletFactoryRegistry::SimpleAccountFactory(
                SIMPLE_ACCOUNT_FACTORY.parse::<Address>().unwrap(),
            )),
            // Test simple account factory address
            "simple-account-test" => Ok(WalletFactoryRegistry::SimpleAccountFactory(
                GETH_SIMPLE_ACCOUNT_FACTORY.parse::<Address>().unwrap(),
            )),
            _ => Err(anyhow::anyhow!("{}'s factory not supported", s)),
        }
    }
}

/// Bundler config
#[derive(Serialize, Deserialize, Debug)]
pub struct BundlerConfig {
    /// Bundler gRPC address to listen on.
    pub bundler_address: IpAddr,
    /// Bundler gRPC port to listen on.
    pub bundler_port: u16,
    /// Bundler EOA wallet seed phrase
    pub bundler_seed: String,
    /// The bundler beneficiary address.
    pub beneficiary_address: Address,
    /// The minimum balance required for the beneficiary address.
    pub min_balance: U256,
    /// The bundle interval in seconds.
    pub bundle_interval: u64,
    /// Sets the send bundle mode.
    pub send_bundle_mode: String,
}

impl Default for BundlerConfig {
    fn default() -> Self {
        Self {
            bundler_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            bundler_port: 3002,
            bundler_seed: "test test test test test test test test test test test junk".to_string(),
            beneficiary_address: Address::zero(),
            min_balance: U256::zero(),
            bundle_interval: 5,
            send_bundle_mode: "eth-client".to_string(),
        }
    }
}

/// UserOperationPool config
#[derive(Serialize, Deserialize, Debug)]
pub struct UserOperationPoolConfig {
    /// UoPool gRPC address to listen on.
    pub uo_pool_address: IpAddr,
    /// UoPool gRPC port to listen on.
    pub uo_pool_port: u16,
    /// Max allowed verification gas.
    pub max_verification_gas: u64,
    /// Minimum stake required for entities.
    pub min_stake: u64,
    /// Minimum unstake delay for entities.
    pub min_unstake_delay: u64,
    /// Minimum priority fee per gas.
    pub min_priority_fee_per_gas: u64,
    /// Addresses of whitelisted entities.
    pub whitelist: Vec<String>,
    /// User operation mempool mode
    pub uopool_mode: String,
}

impl Default for UserOperationPoolConfig {
    fn default() -> Self {
        Self {
            uo_pool_address: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            uo_pool_port: 3003,
            max_verification_gas: 1,
            min_stake: 1,
            min_unstake_delay: 1,
            min_priority_fee_per_gas: 1,
            whitelist: vec![],
            uopool_mode: "standalone".to_string(),
        }
    }
}

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct RpcConfig {
    /// Enables or disables the HTTP RPC.
    pub http: bool,
    /// Sets the HTTP RPC address to listen on.
    pub http_addr: String,
    /// Sets the HTTP RPC port to listen on.
    pub http_port: u16,
    /// Enables or disables the WebSocket RPC.
    pub ws: bool,
    /// Sets the WS RPC address to listen on.
    pub ws_addr: String,
    /// Sets the WS RPC port to listen on.
    pub ws_port: u16,
}
