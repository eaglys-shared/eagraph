use thiserror::Error;

#[derive(Debug, Error)]
pub enum EagraphError {
    #[error("store error: {0}")]
    Store(String),

    #[error("parser error: {0}")]
    Parser(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("symbol not found: {0}")]
    SymbolNotFound(String),

    #[error("repo not found: {0}")]
    RepoNotFound(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, EagraphError>;
