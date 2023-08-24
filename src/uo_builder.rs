use crate::errors::UserOpBuilderError;
use crate::gen::SimpleAccountFactory;
use crate::traits::SmartWalletAccountFactory;
use crate::types::{SignerType, WalletRegistry};
use anyhow::{self, Ok};
use ethers::{
    prelude::{MiddlewareBuilder, SignerMiddleware},
    providers::Middleware,
    signers::{LocalWallet, Signer},
    types::{Address, Bytes, U256},
};
use silius_primitives::UserOperation;
use std::sync::Arc;

#[derive(Debug)]
struct UserOperationBuilder<M: Middleware> {
    /// The chain id
    chain_id: u64,
    /// Ethers provider
    provider: Arc<M>,
    /// The smart contract wallet factory contract object
    factory_contract: Box<dyn SmartWalletAccountFactory<SignerType<M>>>,
    /// The smart contrac wallet factory contract address
    factory_address: Address,
    /// Smart contract wallet address. It's initialized as None.
    scw_address: Option<Address>,
    /// Transaction signer
    signer: Arc<SignerType<M>>,
    /// Signer's wallet address
    signer_address: Address,
    /// Salt used to generated smart contract wallet address
    salt: u64,
    /// `UserOperation`
    uo: UserOperation,
}

impl<M: Middleware> Clone for UserOperationBuilder<M> {
    fn clone(&self) -> Self {
        Self {
            chain_id: self.chain_id,
            provider: self.provider.clone(),
            factory_contract: self.factory_contract.clone_box(),
            factory_address: self.factory_address,
            scw_address: self.scw_address,
            signer: self.signer.clone(),
            signer_address: self.signer_address,
            salt: self.salt.clone(),
            uo: self.uo.clone(),
        }
    }
}

impl<M: Middleware + 'static> UserOperationBuilder<M> {
    /// Create a new UserOperationBuilder
    ///
    /// # Arguments
    /// * `wallet` - The [Wallet](ethers::signers::Wallet) object
    /// * `wallet_name` - The name of the smart wallet the user uses. Ex: SimpleAccount, SoulWallet
    /// * `scw_address` - The smart contract wallet address. None if the user does not have a smat
    /// contract wallet yet.
    /// * `provider` - The ethers provider
    /// * `chain_id` - The chain id
    /// * `factory_address` - The smart contrac wallet factory contract address
    /// * `salt` - The salt used to generate the smart contract wallet address
    ///
    /// # Returns
    /// * `UserOperationBuilder` - The UserOperationBuilder
    /// * `anyhow::Error` - Error
    pub fn new(
        wallet: LocalWallet,
        wallet_name: String,
        scw_address: Option<Address>,
        provider: Arc<M>,
        chain_id: u64,
        factory_address: Address,
        salt: u64,
    ) -> anyhow::Result<UserOperationBuilder<M>> {
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

        let uo = UserOperation::default();

        Ok(Self {
            chain_id,
            provider,
            factory_contract,
            factory_address,
            scw_address,
            signer,
            signer_address,
            salt,
            uo,
        })
    }

    /// Gets the chain id
    pub fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// Gets the factory contract address
    pub fn factory_contract_address(&self) -> Address {
        self.factory_address
    }

    /// Gets the address of the transaction signer
    pub fn signer_address(&self) -> Address {
        self.signer_address
    }

    /// Gets the salt
    pub fn salt(&self) -> u64 {
        self.salt
    }

    /// Gets a reference of the UserOperation
    pub fn uo(&self) -> &UserOperation {
        &self.uo
    }

    /// Generates the smart contract wallet if `self.scw_address` is `None`, otherwise return the
    /// existing `self.scw_address`
    ///
    /// # Arguments
    /// * `salt` - The salt used to generate the smart contract wallet address
    ///
    /// # Returns
    /// * `Address` - The smart contract wallet address
    pub async fn set_scw_address(&mut self) -> anyhow::Result<Address> {
        match self.scw_address {
            Some(address) => Ok({
                log::warn!(
                    "Smart contract wallet address has already been set at: {}",
                    address
                );
                address
            }),
            None => {
                let scw_address = self
                    .factory_contract
                    .generate_address(self.signer_address, U256::from(self.salt))
                    .call()
                    .await?;
                self.scw_address = Some(scw_address);
                Ok(scw_address)
            }
        }
    }

    ////////////////////////////////////////////////////// UserOperation Builder Methods //////////////////////////////////////////////////////

    /// Sets the `sender` of the `UserOperation`
    pub fn set_uo_sender(&mut self, sender: Address) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.sender = sender;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `init_code` of `the UserOperation`
    pub fn set_uo_init_code(&mut self, init_code: Bytes) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.init_code = init_code;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Set the `nonce` of the `UserOperation`
    pub async fn set_uo_nonce(&mut self, nonce: U256) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(address) => Ok({
                let signer_nonce = self.signer.get_transaction_count(address, None).await?;
                if signer_nonce != nonce {
                    log::warn!(
                        "Nonce mismatch: smart contract wallet nonce {} != input nonce {}",
                        signer_nonce,
                        nonce
                    );
                };
                self.uo.nonce = nonce;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `calldata` of the `UserOperation`
    pub fn set_uo_calldata(&mut self, call_data: Bytes) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.call_data = call_data;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `call_gas_limit` of the `UserOperation`
    pub fn set_uo_call_gas_limit(
        &mut self,
        call_gas_limit: U256,
    ) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.call_gas_limit = call_gas_limit;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `verification_gas_limit` of the `UserOperation
    pub fn set_uo_verification_gas_limit(
        &mut self,
        verification_gas_limit: U256,
    ) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.verification_gas_limit = verification_gas_limit;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `pre_verification_gas` of the `UserOperation
    pub fn set_uo_pre_verification_gas(
        &mut self,
        pre_verification_gas: U256,
    ) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.pre_verification_gas = pre_verification_gas;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `max_fee_per_gas` of the `UserOperation
    pub fn set_uo_max_fee_per_gas(
        &mut self,
        max_fee_per_gas: U256,
    ) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.max_fee_per_gas = max_fee_per_gas;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `max_priority_fee_per_gas` of the `UserOperation
    pub fn set_uo_max_priority_fee_per_gas(
        &mut self,
        max_priority_fee_per_gas: U256,
    ) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.max_priority_fee_per_gas = max_priority_fee_per_gas;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `paymaster_and_data` of the `UserOperation
    pub fn set_uo_paymaster_and_data(
        &mut self,
        paymaster_and_data: Bytes,
    ) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.paymaster_and_data = paymaster_and_data;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }

    /// Sets the `signature` of the `UserOperation`
    pub fn set_uo_signature(&mut self, signature: Bytes) -> anyhow::Result<&UserOperation> {
        match self.scw_address {
            Some(_) => Ok({
                self.uo.signature = signature;
                &self.uo
            }),
            None => Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::SmartContractWalletAddressNotSet
            )),
        }
    }
}
