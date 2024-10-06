use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("KART token is not enough to stake")]
    InsufficientToken {},

    #[error("Stake amount need to be greater than 0")]
    InvalidAmount {},

    #[error("Token is not supported")]
    UnsupportedToken {},

    #[error("Custom Error val: {msg:?}")]
    CustomError { msg: String },
    // Add any other custom errors you like here.
    // Look at https://docs.rs/thiserror/1.0.21/thiserror/ for details.
}
