use thiserror::Error;

pub const ACTIVATION_HEIGHT: u32 = 100;
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
