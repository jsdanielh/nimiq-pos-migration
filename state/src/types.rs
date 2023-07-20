use nimiq_genesis_builder::config::{GenesisAccount, GenesisHTLC, GenesisVestingContract};

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
