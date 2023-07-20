pub mod types;

use nimiq_genesis_builder::config::{GenesisConfig, GenesisValidator};
use nimiq_rpc::Client;
use thiserror::Error;

use crate::types::GenesisAccounts;

/// Error types that can be returned
#[derive(Error, Debug)]
pub enum Error {
    /// RPC error
    #[error("RPC error: {0}")]
    Rpc(#[from] jsonrpc::Error),
    /// Unknown PoW block
    #[error("Unknown PoW block")]
    UnknownBlock,
}

/// Gets the PoS genesis history root by getting all of the transactions from the
/// PoW chain and building a single history tree.
pub fn get_accounts(
    client: Client,
    block_hash: String,
    block_number: u32,
) -> Result<GenesisAccounts, Error> {
    unimplemented!();
}

/// Gets the PoS genesis history root by getting all of the transactions from the
/// PoW chain and building a single history tree.
pub fn get_validators(
    client: Client,
    block_hash: String,
    block_number: u32,
) -> Result<Vec<GenesisValidator>, Error> {
    unimplemented!();
}

/// Gets the genesis config file
pub fn get_pos_genesis(
    client: Client,
    block_hash: String,
    block_number: u32,
) -> Result<GenesisConfig, Error> {
    unimplemented!()
}

/// Write the genesis config file to a TOML file
pub fn write_pos_genesis(file_name: &str, genesis_config: GenesisConfig) -> Result<(), Error> {
    unimplemented!()
}
