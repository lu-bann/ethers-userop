use crate::traits::SmartWalletAccountFactory;
use alloy_sol_types::sol;
use ethers::{
    contract::abigen,
    prelude::FunctionCall,
    providers::Middleware,
    types::{Address, U256},
};

use std::sync::Arc;

abigen!(SimpleAccountFactory, "src/abi/SimpleAccountFactory.json",);

abigen!(SimpleAccount, "src/abi/SimpleAccount.json",);

abigen!(EntryPoint, "src/abi/EntryPoint.json",);

sol! {function execute(address dest, uint256 value, bytes calldata func);}

impl<M: Middleware + 'static> SmartWalletAccountFactory<M> for SimpleAccountFactory<M> {
    /// create an account, and return its address
    /// returns the address even if the account is already deployed
    /// Note that during UserOperation execution, this method is called only if the account is not deployed.
    fn create_account(
        &self,
        creator_address: Address,
        salt: U256,
    ) -> FunctionCall<Arc<M>, M, Address> {
        self.create_account(creator_address, salt)
    }

    /// calculate the counterfactual address of this account given a salt
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
