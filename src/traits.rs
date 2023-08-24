use core::fmt::Debug;
use ethers::{
    prelude::FunctionCall,
    providers::Middleware,
    types::{Address, H160, U256},
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

    /// Implementing the Clone trait for trait
    fn clone_box(&self) -> Box<dyn SmartWalletAccountFactory<M>>;
}

pub trait SmartWalletAccount {
    /// Executes a transaction (called direcly from the owner or entryPoint)
    fn execute(&self, dest: Address, value: U256, func: Vec<u8>) -> anyhow::Result<()>;
}
