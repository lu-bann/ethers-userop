use crate::traits::{SmartWalletAccount, SmartWalletAccountFactory};
use alloy_primitives::{Address as a_Address, U256 as a_U256};
use alloy_sol_types::sol;
use alloy_sol_types::SolCall;
use ethers::{
    contract::abigen,
    prelude::FunctionCall,
    providers::Middleware,
    types::{Address, Bytes, U256},
};

use std::sync::Arc;

abigen!(SimpleAccountFactory, "src/abi/SimpleAccountFactory.json",);

abigen!(SimpleAccount, "src/abi/SimpleAccount.json",);

abigen!(EntryPoint, "src/abi/EntryPoint.json",);

// Simple account `execute()` function. See https://github.com/eth-infinitism/account-abstraction/blob/75f02457e71bcb4a63e5347589b75fa4da5c9964/contracts/samples/SimpleAccount.sol#L67
sol! {function execute(address dest, uint256 value, bytes calldata func);}
pub struct SimpleAccountExecute(executeCall);
impl SimpleAccountExecute {
    pub fn new(address: Address, value: U256, func: Bytes) -> Self {
        Self(executeCall {
            dest: a_Address::from(address.0),
            value: a_U256::from_limbs(value.0),
            func: func.to_vec(),
        })
    }

    /// Encodes the calldata
    pub fn encode(&self) -> Vec<u8> {
        self.0.encode()
    }
}

impl<M: Middleware + 'static> SmartWalletAccountFactory<M> for SimpleAccountFactory<M> {
    /// create an account, and return its address
    /// returns the address even if the account is already deployed
    /// Note that during UserOperation execution, this method is called only if the account is not deployed.
    ///
    /// # Arguments
    /// * `creator_address` - The address of the user that creates the account
    /// * `salt` - The salt
    ///
    /// # Returns
    /// * `FunctionCall` - The function call
    fn create_account(
        &self,
        creator_address: Address,
        salt: U256,
    ) -> FunctionCall<Arc<M>, M, Address> {
        self.create_account(creator_address, salt)
    }

    /// calculate the counterfactual address of this account given a salt
    ///
    /// # Arguments
    /// * `creator_address` - The address of the user that creates the account
    /// * `salt` - The salt
    ///
    /// # Returns
    /// * `FunctionCall` - The function call
    fn generate_address(
        &self,
        creator_address: Address,
        salt: U256,
    ) -> FunctionCall<Arc<M>, M, Address> {
        self.get_address(creator_address, salt)
    }

    fn clone_box(&self) -> Box<dyn SmartWalletAccountFactory<M>> {
        Box::new(self.clone())
    }
}

impl<M: Middleware + 'static> SmartWalletAccount for SimpleAccount<M> {
    /// Executes a transaction (called direcly from the owner or entryPoint)
    ///
    /// # Arguments
    /// * `dest` - The destination address
    /// * `value` - The value sent
    /// * `func` - The function signature
    ///
    /// # Returns
    /// * `Result<()> - None
    fn execute(&self, dest: Address, value: U256, func: Bytes) -> Vec<u8> {
        let sae = SimpleAccountExecute::new(dest, value, func);
        sae.0.encode()
    }

    fn clone_box(&self) -> Box<dyn SmartWalletAccount> {
        Box::new(self.clone())
    }
}
