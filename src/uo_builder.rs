use crate::errors::UserOpBuilderError;
use crate::gen::{SimpleAccount, SimpleAccountFactory};
use crate::traits::SmartWalletAccount;
use crate::traits::SmartWalletAccountFactory;
use crate::types::{WalletFactoryRegistry, WalletRegistry};
use anyhow::Ok;
use ethers::{
    providers::Middleware,
    types::{Address, Bytes, U256},
};
use silius_primitives::{UserOperation, UserOperationHash, UserOperationPartial};
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug)]
pub struct UserOperationBuilder<M: Middleware> {
    /// Ethers provider
    provider: Arc<M>,
    /// The smart contract wallet factory contract object
    factory_contract: Box<dyn SmartWalletAccountFactory<M>>,
    /// The smart contrac wallet factory contract address
    factory_address: Address,
    /// The smart contract wallet contract object
    wallet_contract: Box<dyn SmartWalletAccount>,
    /// Smart contract wallet address. It's initialized as None.
    scw_address: Option<Address>,
    /// Signer's wallet address
    signer_address: Address,
    /// Salt used to generated smart contract wallet address
    salt: Option<u64>,
    /// [UserOperationPartial](silius_primitives::UserOperationPartial)
    uo: UserOperationPartial,
    /// The [hash of a UserOperation](silius_primitives::UserOperation::hash)
    uo_hash: Option<UserOperationHash>,
}

impl<M: Middleware> Clone for UserOperationBuilder<M> {
    fn clone(&self) -> Self {
        Self {
            provider: self.provider.clone(),
            factory_contract: self.factory_contract.clone_box(),
            factory_address: self.factory_address,
            wallet_contract: self.wallet_contract.clone_box(),
            scw_address: self.scw_address,
            signer_address: self.signer_address,
            salt: self.salt,
            uo: self.uo.clone(),
            uo_hash: self.uo_hash,
        }
    }
}

impl<M: Middleware + 'static> UserOperationBuilder<M> {
    /// Create a new UserOperationBuilder
    ///
    /// # Arguments
    /// * `wallet_address` - The wallet address
    /// * `wallet_name` - The name of the smart wallet the user uses. Ex: SimpleAccount, SoulWallet
    /// * `scw_address` - The smart contract wallet address. None if the user does not have a smat
    /// contract wallet yet.
    /// * `provider` - The ethers provider
    /// * `chain_id` - The chain id
    /// * `salt` - The salt used to generate the smart contract wallet address
    ///
    /// # Returns
    /// * `UserOperationBuilder` - The UserOperationBuilder
    /// * `anyhow::Error` - Error
    pub fn new(
        eoa_wallet_address: Address,
        wallet_name: impl Into<String>,
        scw_address: Option<Address>,
        provider: Arc<M>,
        salt: Option<u64>,
    ) -> anyhow::Result<Self> {
        // Dynamic dispatch to decide which wallet to initiate
        let (wallet_contract, factory_contract, factory_address) =
            Self::match_wallet(wallet_name.into(), provider.clone())?;

        let uo = UserOperationPartial {
            sender: None,
            nonce: None,
            init_code: None,
            call_data: None,
            call_gas_limit: None,
            verification_gas_limit: None,
            pre_verification_gas: None,
            max_fee_per_gas: None,
            max_priority_fee_per_gas: None,
            paymaster_and_data: None,
            signature: None,
        };

        Ok(Self {
            provider,
            factory_contract,
            factory_address,
            wallet_contract,
            scw_address,
            signer_address: eoa_wallet_address,
            salt,
            uo,
            uo_hash: None,
        })
    }

    /// initialize a UserOperationBuilder with a [UserOperationPartial](silius_primitives::UserOperationPartial)
    pub fn from_uo(
        uo: UserOperationPartial,
        provider: Arc<M>,
        wallet_name: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let mut uo_builder = Self::new(Address::zero(), wallet_name, None, provider, None)?;
        uo_builder.set_uo(uo);
        Ok(uo_builder)
    }

    /// Dynamic dispatch to decide which wallet to initiate given a wallet name
    ///
    /// # Arguments
    /// * `wallet_name` - The name of the smart wallet the user uses. Ex: SimpleAccount, SoulWallet
    /// * `provider` - The ethers provider
    ///
    /// # Returns
    /// * `Box<dyn SmartWalletAccount>` - The smart contract wallet contract object
    /// * `Box<dyn SmartWalletAccountFactory<M>>` - The smart contract wallet factory contract object
    /// * `Address` - The smart contract wallet factory contract address
    #[allow(clippy::type_complexity)]
    fn match_wallet(
        wallet_name: String,
        provider: Arc<M>,
    ) -> anyhow::Result<(
        Box<dyn SmartWalletAccount>,
        Box<dyn SmartWalletAccountFactory<M>>,
        Address,
    )> {
        let (factory_contract, factory_address): (Box<dyn SmartWalletAccountFactory<M>>, Address) =
            match WalletFactoryRegistry::from_str(&wallet_name)? {
                WalletFactoryRegistry::SimpleAccountFactory(addr) => {
                    let wf = Box::new(SimpleAccountFactory::new(addr, provider.clone()));
                    (wf, addr)
                }
            };
        let wallet_contract: Box<dyn SmartWalletAccount> =
            match WalletRegistry::from_str(&wallet_name)? {
                WalletRegistry::SimpleAccount => {
                    Box::new(SimpleAccount::new(factory_address, provider.clone())) as _
                }
            };
        Ok((wallet_contract, factory_contract, factory_address))
    }

    /// Gets the factory contract address
    pub fn factory_contract_address(&self) -> Address {
        self.factory_address
    }

    /// Gets the factory contract struct
    pub fn factory_contract(&self) -> Box<dyn SmartWalletAccountFactory<M>> {
        self.factory_contract.clone_box()
    }

    /// Gets the walelt contract struct
    pub fn wallet_contract(&self) -> Box<dyn SmartWalletAccount> {
        self.wallet_contract.clone_box()
    }

    /// Gets the address of the transaction signer
    pub fn signer_address(&self) -> Address {
        self.signer_address
    }

    /// Gets the address of the smat contract wallet
    /// None if not yet deployed
    pub fn scw_address(&self) -> Option<Address> {
        self.scw_address
    }

    /// Gets the salt
    pub fn salt(&self) -> Option<u64> {
        self.salt
    }

    /// Gets a reference of the UserOperation
    pub fn uo(&self) -> &UserOperationPartial {
        &self.uo
    }

    /// Gets the user operation hash
    pub fn uo_hash(&self) -> &Option<UserOperationHash> {
        &self.uo_hash
    }

    /// Generates the smart contract wallet if `self.scw_address` is `None`, otherwise return the
    /// existing `self.scw_address`
    ///
    /// # Returns
    /// * `Address` - The smart contract wallet address
    pub async fn set_scw_address(&mut self) -> anyhow::Result<Address> {
        let scw_address = self
            .factory_contract
            .generate_address(
                self.signer_address,
                U256::from(self.salt.expect("salt is None")),
            )
            .call()
            .await?;
        self.scw_address = Some(scw_address);
        Ok(scw_address)
    }

    ////////////////////////////////////////////////////// UserOperation Builder Methods //////////////////////////////////////////////////////

    /// Sets the UserOperation of the UserOperationBuilder
    pub fn set_uo(&mut self, uo: UserOperationPartial) -> &mut Self {
        self.uo = uo;
        self
    }

    /// Updates the `UserOperationBuilder` with a new wallet, which will update the wallet contract, factory contract, and factory
    pub fn set_wallet(&mut self, wallet_name: String) -> anyhow::Result<&mut Self> {
        let (wallet_contract, factory_contract, factory_address) =
            Self::match_wallet(wallet_name, self.provider.clone())?;
        self.wallet_contract = wallet_contract;
        self.factory_contract = factory_contract;
        self.factory_address = factory_address;
        Ok(self)
    }

    /// Sets the `sender` of the `UserOperation`
    ///
    /// # Arguments
    /// * `sender` - The sender address
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_sender(&mut self, sender: Address) -> &mut Self {
        self.uo.sender = Some(sender);
        self
    }

    /// Sets the `init_code` of `the UserOperation`
    ///
    /// # Arguments
    /// * `init_code` - The init code used for contract deployment
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_init_code(&mut self, init_code: Bytes) -> &mut Self {
        self.uo.init_code = Some(init_code);
        self
    }

    /// Set the `nonce` of the `UserOperation`
    ///
    /// # Arguments
    /// * `nonce` - The smart account's nonce
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_nonce(&mut self, nonce: U256) -> &mut Self {
        self.uo.nonce = Some(nonce);
        self
    }

    /// Sets the `calldata` of the `UserOperation`
    ///
    /// # Arguments
    /// * `call_data` - The calldata
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_calldata(&mut self, call_data: Bytes) -> &mut Self {
        self.uo.call_data = Some(call_data);
        self
    }

    /// Sets the `call_gas_limit` of the `UserOperation`
    ///
    /// # Arguments
    /// * `call_gas_limit` - The call gas limit that's metered during the exeuction
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_call_gas_limit(&mut self, call_gas_limit: U256) -> &mut Self {
        self.uo.call_gas_limit = Some(call_gas_limit);
        self
    }

    /// Sets the `verification_gas_limit` of the `UserOperation
    ///
    /// #Arguments
    /// * `verification_gas_limit` - The verification gas limit that's metered during verification
    ///
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_verification_gas_limit(&mut self, verification_gas_limit: U256) -> &mut Self {
        self.uo.verification_gas_limit = Some(verification_gas_limit);
        self
    }

    /// Sets the `pre_verification_gas` of the `UserOperation
    ///
    /// # Arguments
    /// * `pre_verification_gas` - The pre verification gas that's overhead cost for each
    /// `UserOperation`
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_pre_verification_gas(&mut self, pre_verification_gas: U256) -> &mut Self {
        self.uo.pre_verification_gas = Some(pre_verification_gas);
        self
    }

    /// Sets the `max_fee_per_gas` of the `UserOperation
    ///
    /// # Arguments
    /// * `max_fee_per_gas` - The max fee per gas similar to the `gas limit` in a regular
    /// transaction
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_max_fee_per_gas(&mut self, max_fee_per_gas: U256) -> &mut Self {
        self.uo.max_fee_per_gas = Some(max_fee_per_gas);
        self
    }

    /// Sets the `max_priority_fee_per_gas` of the `UserOperation
    ///
    /// # Arguments
    /// * `max_priority_fee_per_gas` - The max priority fee per gas, similar to "tips" in a
    /// regular transaction
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_max_priority_fee_per_gas(&mut self, max_priority_fee_per_gas: U256) -> &mut Self {
        self.uo.max_priority_fee_per_gas = Some(max_priority_fee_per_gas);
        self
    }

    /// Sets the `paymaster_and_data` of the `UserOperation
    ///
    /// # Arguments
    /// * `paymaster_and_data` - The paymaster address and additional data
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_paymaster_and_data(&mut self, paymaster_and_data: Bytes) -> &mut Self {
        self.uo.paymaster_and_data = Some(paymaster_and_data);
        self
    }

    /// Sets the `signature` of the `UserOperation`
    ///
    /// # Arguments
    /// * `signature` - The signature
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub fn set_uo_signature(&mut self, signature: Bytes) -> &mut Self {
        self.uo.signature = Some(signature);
        self
    }

    /// Sets the `uo_hash` of the [UserOperation](silius_primitives::UserOperation) when the [UserOperation](silus_primitives::UserOperation) is successfully sent
    ///
    /// # Arguments
    /// * `uo_hash` - The [hash of a UserOperation](silius_primitives::UserOperation::hash)
    ///
    /// # Returns
    /// * `&mut Self` - Self
    pub(crate) fn set_uo_hash(&mut self, uo_hash: UserOperationHash) -> &mut Self {
        self.uo_hash = Some(uo_hash);
        self
    }

    /// Build a UserOperation to send to the bundler by checking all the fields are non-empty
    ///
    /// # Return
    /// * `UserOperation` - The [UserOperation](silius_primitives::UserOperation) to be sent
    pub fn build_uo(&self) -> anyhow::Result<UserOperation> {
        if self.uo.sender.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("sender".to_string())
            ));
        };
        if self.uo.nonce.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("nonce".to_string())
            ));
        };
        if self.uo.init_code.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("init_code".to_string())
            ));
        };
        if self.uo.call_data.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("call_data".to_string())
            ));
        };
        if self.uo.call_gas_limit.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("call_gas_limit".to_string())
            ));
        };
        if self.uo.pre_verification_gas.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField(
                    "pre_verification_gas".to_string()
                )
            ));
        };
        if self.uo.verification_gas_limit.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField(
                    "verification_gas_limit".to_string()
                )
            ));
        };
        if self.uo.max_priority_fee_per_gas.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField(
                    "max_priority_fee_per_gas".to_string()
                )
            ));
        };
        if self.uo.max_fee_per_gas.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("max_fee_per_gas".to_string())
            ));
        };
        if self.uo.paymaster_and_data.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField(
                    "paymaster_and_data".to_string()
                )
            ));
        };
        if self.uo.signature.is_none() {
            return Err(anyhow::anyhow!(
                UserOpBuilderError::<M>::MissingUserOperationField("signature".to_string())
            ));
        };

        let uo = UserOperation::from(self.uo.clone());

        Ok(uo)
    }
}
