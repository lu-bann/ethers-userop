use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};
use ethers::{
    prelude::{NonceManagerMiddleware, SignerMiddleware},
    providers::{Http, Middleware, Provider},
    signers::LocalWallet,
    types::{Address as EAddress, Bytes as EBytes, U256 as EU256},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// In most smart contract account implementations the signature field is computed off-chain by hashing a user operation and signing that hash using some signature scheme
/// This signature must be computed after gas is estimated, as those fields are included in the hash. However, there are portions of the gas estimation step that require the signature field to be populated: preVerificationGas and verificationGasLimit.
/// To get around this, we use a dummy signature that is the same length as a real signature, but is not a valid signature. This allows us to compute the gas estimation for `preVerificationGas` and `verificationGasLimit` without real signature.
/// See https://www.alchemy.com/blog/dummy-signatures-and-gas-token-transfers
pub const DUMMY_PAYMASTER_AND_DATA: &str = "0xC03Aac639Bb21233e0139381970328dB8bcEeB67fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0000000000000000000000000000000007aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1c";
pub const DUMMY_SIGNATURE: &str = "0xfffffffffffffffffffffffffffffff0000000000000000000000000000000007aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1c";

/// Entry point contract address. All entry point contracts have the same address
pub const ENTRY_POINT_ADDRESS: &str = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789";
/// Deployed entry point contract address on Geth testnet
pub const GETH_ENTRY_POINT_ADDRESS: &str = "0x5fbdb2315678afecb367f032d93f642f64180aa3 ";
/// stackup simple account factory
pub const SIMPLE_ACCOUNT_FACTORY: &str = "0x9406Cc6185a346906296840746125a0E44976454";
/// Deployed simple account factory on Geth testnet
pub const GETH_SIMPLE_ACCOUNT_FACTORY: &str = "0xe7f1725e7734ce288f8367e1bb143e90bb3f0512";
/// SALT used when creating a new smart contract wallet
pub const SALT: u64 = 2;
/// Test Key phrase
pub const SEED_PHRASE: &str = "test test test test test test test test test test test junk";
/// RPC namespaces
pub const RPC_NAMESPACE: [&str; 2] = ["eth", "debug"];
/// Geth Testnet chain id
pub const GETH_CHAIN_ID: u64 = 1337;
/// Goerli chain id
pub const GOERLI_CHAIN_ID: u64 = 1337;

/// Nonce manager middleware type alias
pub type ClientType = NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>;

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
sol! {function execute(address dest, uint256 value, bytes calldata func);}
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
