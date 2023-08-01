use hex::FromHexError;
use nimiq_genesis_builder::config::{GenesisAccount, GenesisHTLC, GenesisVestingContract};
use nimiq_keys::AddressParseError;
use nimiq_primitives::coin::Coin;
use nimiq_primitives::coin::CoinConvertError;
use thiserror::Error;

/// Error types that can be returned
#[derive(Error, Debug)]
pub enum Error {
    /// RPC error
    #[error("RPC error: {0}")]
    Rpc(#[from] jsonrpc::Error),
    /// Address parsing error
    #[error("Failed to parse Nimiq address")]
    Address(#[from] AddressParseError),
    /// Coin conversion error
    #[error("Failed to convert to coin")]
    Coin(#[from] CoinConvertError),
    /// Hex conversion error
    #[error("Failed to decode string as hex")]
    Hex(#[from] FromHexError),
    /// Invalid value
    #[error("Invalid value")]
    InvalidValue,
}

/// Genesis accounts for the genesis state
#[derive(Debug)]
pub struct GenesisAccounts {
    /// Basic accounts for the genesis state.
    pub basic_accounts: Vec<GenesisAccount>,

    /// Vesting accounts for the genesis state.
    pub vesting_accounts: Vec<GenesisVestingContract>,

    /// HTLC accounts for the genesis state.
    pub htlc_accounts: Vec<GenesisHTLC>,
}

/// Genesis validators for the genesis state
#[derive(Clone, Debug)]
pub struct GenesisValidator {
    /// Inner genesis validator information
    pub validator: nimiq_genesis_builder::config::GenesisValidator,

    /// Validator stake
    pub balance: Coin,
}
