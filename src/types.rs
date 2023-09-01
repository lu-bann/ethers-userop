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

impl std::str::FromStr for WalletRegistry {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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

impl std::str::FromStr for WalletFactoryRegistry {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
