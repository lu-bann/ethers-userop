use crate::gen::SimpleAccountFactory;
use crate::traits::SmartWalletAccountFactory;
use crate::types::{SignerType, WalletRegistry};
use anyhow::{self, Ok};
use ethers::{
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::Middleware,
    signers::{coins_bip39::English, MnemonicBuilder, Signer},
    types::{Address, U256},
};
use std::sync::Arc;

#[derive(Debug)]
struct UserOperationBuilder<M: Middleware> {
    /// The bundler RPC url
    rpc_address: String,
    /// The chain id
    chain_id: u64,
    /// Ethers provider
    provider: Arc<M>,
    /// The smart contract wallet factory contract object
    factory_contract: Box<dyn SmartWalletAccountFactory<SignerType<M>>>,
    /// The smart contrac wallet factory contract address
    factory_address: Address,
    /// Smart contract wallet address. It's initialized as None.
    swc_address: Option<Address>,
    /// Transaction signer
    signer: Arc<SignerType<M>>,
    /// Signer's wallet address
    signer_address: Address,
}

impl<M: Middleware> Clone for UserOperationBuilder<M> {
    fn clone(&self) -> Self {
        Self {
            rpc_address: self.rpc_address.clone(),
            chain_id: self.chain_id,
            provider: self.provider.clone(),
            factory_contract: self.factory_contract.clone_box(),
            factory_address: self.factory_address,
            swc_address: self.swc_address,
            signer: self.signer.clone(),
            signer_address: self.signer_address,
        }
    }
}

impl<M: Middleware + 'static> UserOperationBuilder<M> {
    /// Create a new UserOperationBuilder
    ///
    /// # Arguments
    /// * `rpc_address` - The bundler RPC url
    /// * `seed_phrase` - The seed phrase for the wallet
    /// * `wallet_name` - The wallet the user uses. Ex: SimpleAccount, SoulWallet
    /// * `provider` - The ethers provider
    /// * `chain_id` - The chain id
    /// * `factory_address` - The smart contrac wallet factory contract address
    /// * `factory_contract_binding` - The factory contract binding
    ///
    /// # Returns
    /// * `UserOperationBuilder` - The UserOperationBuilder
    /// * `anyhow::Error` - Error
    fn new(
        rpc_address: String,
        seed_phrase: String,
        wallet_name: String,
        provider: Arc<M>,
        chain_id: u64,
        factory_address: Address,
    ) -> anyhow::Result<UserOperationBuilder<M>> {
        let wallet = MnemonicBuilder::<English>::default()
            .phrase(seed_phrase.clone().as_str())
            .build()?;
        let signer_address = wallet.address();
        let client =
            SignerMiddleware::new(provider.clone(), wallet.clone().with_chain_id(chain_id))
                .nonce_manager(wallet.clone().address());
        let signer: Arc<SignerType<M>> = Arc::new(client);

        let factory_contract: Box<dyn SmartWalletAccountFactory<SignerType<M>>> =
            match WalletRegistry::from_str(&wallet_name)? {
                WalletRegistry::SimpleAccount => {
                    let wf = Box::new(SimpleAccountFactory::new(factory_address, signer.clone()));
                    wf
                }
            };

        Ok(Self {
            rpc_address,
            chain_id,
            provider,
            factory_contract,
            factory_address,
            swc_address: None,
            signer,
            signer_address,
        })
    }

    /// Gets the bundler RPC listening address
    fn get_rpc_address(&self) -> &String {
        &self.rpc_address
    }

    /// Gets the chain id
    fn get_chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Gets the factory contract address
    fn get_factory_contract_address(&self) -> Address {
        self.factory_address
    }

    /// Gets the address of the transaction signer
    fn get_signer_address(&self) -> Address {
        self.signer_address
    }

    /// Generates the smart contract wallet given the salt
    ///
    /// # Arguments
    /// * `salt` - The salt used to generate the smart contract wallet address
    ///
    /// # Returns
    /// * `Address` - The smart contract wallet address
    async fn generate_swc_address(&self, salt: u64) -> anyhow::Result<Address> {
        let swc_address = self
            .factory_contract
            .generate_address(self.signer_address, U256::from(salt))
            .call()
            .await?;

        Ok(swc_address)
    }
}
