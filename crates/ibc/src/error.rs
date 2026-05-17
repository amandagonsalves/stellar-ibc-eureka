use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("IBC handler error: {0}")]
    Handler(#[from] Box<ibc::core::handler::types::error::HandlerError>),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("decoding error: {0}")]
    Decoding(String),

    #[error("client error: {0}")]
    Client(String),

    #[error("{0}")]
    Other(String),
}
