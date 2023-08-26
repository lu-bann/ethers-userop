use ethers::signers::{coins_bip39::English, LocalWallet, MnemonicBuilder};

/// Given a seed phrase, build a wallet
///
/// # Arguments
/// * `seed` - The seed phrase
///
/// # Returns
/// * `Wallet<SigningKey>` - The wallet
pub fn build_wallet(seed: &str) -> anyhow::Result<LocalWallet> {
    let wallet = MnemonicBuilder::<English>::default().phrase(seed).build()?;
    Ok(wallet)
}
