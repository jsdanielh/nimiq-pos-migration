use thiserror::Error;

pub const ACTIVATION_HEIGHT: u64 = 100;
pub const BURN_ADDRESS: &str = "NQ07 0000 0000 0000 0000 0000 0000 0000 0000";
pub enum ValidatorsReadiness {
    NotReady(u16),
    Ready(u16),
}

#[derive(Error, Debug)]
pub enum Error {
    /// RPC error
    #[error("RPC error")]
    Rpc,
}
