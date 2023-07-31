use nimiq_genesis_builder::config::{GenesisAccount, GenesisHTLC, GenesisVestingContract};
use nimiq_primitives::coin::Coin;

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
pub struct GenesisValidator {
    /// Inner genesis validator information
    pub validator: nimiq_genesis_builder::config::GenesisValidator,

    /// Validator stake
    pub balance: Coin,
}
