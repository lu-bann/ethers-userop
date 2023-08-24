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
