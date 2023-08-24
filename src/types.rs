use crate::gen::executeCall;
use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use ethers::{
    prelude::{NonceManagerMiddleware, SignerMiddleware},
    providers::Middleware,
    signers::LocalWallet,
    types::{Address as EAddress, Bytes as EBytes, U256 as EU256},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;

/// Nonce manager middleware type alias
pub type SignerType<M> = NonceManagerMiddleware<SignerMiddleware<Arc<M>, LocalWallet>>;

// Error thrown when the UserOpMiddleware interacts with the bundlers
#[derive(Debug, Clone, Error)]
pub enum UserOpMiddlewareError<M: Middleware> {
    /// Thrown when the internal middleware errors
    #[error("Middleware error: {0}")]
    MiddlewareError(M::Error),
}
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
    pub pre_verification_gas: EU256,
    pub verification_gas_limit: EU256,
    pub call_gas_limit: EU256,
}

#[derive(Debug, Deserialize)]
pub struct Response<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub result: T,
}

// Simple account `execute()` function. See https://github.com/eth-infinitism/account-abstraction/blob/75f02457e71bcb4a63e5347589b75fa4da5c9964/contracts/samples/SimpleAccount.sol#L67
pub struct SimpleAccountExecute(executeCall);
impl SimpleAccountExecute {
    pub fn new(address: EAddress, value: EU256, func: EBytes) -> Self {
        Self(executeCall {
            dest: Address::from(address.0),
            value: U256::from_limbs(value.0),
            func: func.to_vec(),
        })
    }

    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }
}
pub struct DeployedContract<C> {
    contract: C,
    pub address: EAddress,
}
impl<C> DeployedContract<C> {
    pub fn new(contract: C, addr: EAddress) -> Self {
        Self {
            contract,
            address: addr,
        }
    }

    pub fn contract(&self) -> &C {
        &self.contract
    }
}

pub enum WalletRegistry {
    SimpleAccount,
}

impl WalletRegistry {
    pub fn from_str(s: &str) -> anyhow::Result<WalletRegistry> {
        match s {
            "simple_account" => Ok(WalletRegistry::SimpleAccount),
            _ => Err(anyhow::anyhow!("Unknown wallet registry type")),
        }
    }
}
