use hex::FromHexError;
use nimiq_genesis_builder::config::{GenesisAccount, GenesisHTLC, GenesisVestingContract};
use nimiq_keys::AddressParseError;
use nimiq_primitives::coin::Coin;
use nimiq_primitives::coin::CoinConvertError;
use thiserror::Error;
use time::error::ComponentRange;

/// Error types that can be returned
#[derive(Error, Debug)]
pub enum Error {
    /// RPC error
    #[error("RPC error: {0}")]
    Rpc(#[from] jsonrpc::Error),
    /// Unknown PoW block
    #[error("Unknown PoW block")]
    UnknownBlock,
    /// IO error
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    /// Serialization error
    #[error("Serialization: {0}")]
    Serialization(#[from] toml::ser::Error),
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
    /// Invalid time
    #[error("Invalid timestamp")]
    Timestamp(#[from] ComponentRange),
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

/// PoW registration window
///
/// The registration window is a set of blocks in the PoW chain that marks
/// the start and end of different windows as follows:
///
///     1                  2                  3                  4         5
/// --- | ---------------- | ---------------- | ---------------- | ------- |
///
/// 1. Validator registration window start block.
/// 2. Validator registration window end block. This block is also the pre-stake
///    registration window start.
/// 3. Pre-stake registration window end block. This block is also the activation
///    window start.
/// 4. The final block in the PoW chain that will be taken as genesis block for the
///    PoS chain. This block must have a block number that can be an election block
///    number in the PoS chain.
/// 5. This is a block whose block number is a number of confirmations away from
///    the final block described in 4.
///
#[derive(Debug)]
pub struct PoWRegistrationWindow {
    /// Block hash of the validator registration window start
    pub validator_start: String,
    /// Block hash of the validator registration window end which is also
    /// the pre stake registration window start.
    pub pre_stake_start: String,
    /// Pre stake registration window stop.
    pub pre_stake_end: String,
    /// Final block in the PoW chain that will be taken as the genesis block for
    /// the PoS chain.
    pub final_block: String,
    /// Number of confirmations after the final block needed for the PoS chain to
    /// start.
    pub confirmations: u32,
}
