use ethers::providers::Middleware;
use thiserror::Error;

// Error thrown when the UserOpMiddleware interacts with the bundlers
#[derive(Debug, Clone, Error)]
pub enum UserOpMiddlewareError<M: Middleware> {
    /// Thrown when the internal middleware errors
    #[error("Middleware error: {0}")]
    MiddlewareError(M::Error),
}

// Error thrown when the UserOpbuilder constructs the UserOperation
#[derive(Error, Debug)]
pub enum UserOpBuilderError<M: Middleware> {
    /// Thrown when the internal middleware errors
    #[error("Middleware error: {0}")]
    MiddlewareError(M::Error),
    /// Thrown when the smart contract wallet address has not been set
    #[error("Smart contract wallet address has not been set")]
    SmartContractWalletAddressNotSet,
}
