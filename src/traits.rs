use alloy_primitives::{Address as a_Address, U256 as a_U256};
use alloy_sol_types::sol;
use alloy_sol_types::SolCall;
use core::fmt::Debug;
use ethers::{
    prelude::FunctionCall,
    providers::Middleware,
    types::{Address, Bytes, H160, U256},
};
use std::sync::Arc;
pub trait SmartWalletAccountFactory<M: Middleware>: Debug {
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
    fn create_account(&self, creator_address: Address, salt: U256)
        -> FunctionCall<Arc<M>, M, H160>;

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
    ) -> FunctionCall<Arc<M>, M, H160>;

    /// Implementing the Clone trait for UserOperationBuilder
    fn clone_box(&self) -> Box<dyn SmartWalletAccountFactory<M>>;
}

sol! {function execute(address dest, uint256 value, bytes calldata func);}
pub trait SmartWalletAccount: Debug + Send {
    /// Executes a transaction (called direcly from the owner or entryPoint)
    /// Default implementation provided
    ///
    /// # Arguments
    /// * `dest` - The destination address
    /// * `value` - The value sent
    /// * `func` - The function signature
    ///
    /// # Returns
    /// * `Vec<u8> - The encoded calldata
    fn execute(&self, dest: Address, value: U256, func: Bytes) -> Vec<u8> {
        let call = executeCall {
            dest: a_Address::from(dest.0),
            value: a_U256::from_limbs(value.0),
            func: func.to_vec(),
        };
        call.encode()
    }

    /// Implementing the Clone trait for UserOperationBuilder
    fn clone_box(&self) -> Box<dyn SmartWalletAccount>;
}
