use thiserror::Error;

/// Errors returned by discovery and source resolution.
///
/// [`DiscoveryError::Config`] means the caller supplied an invalid network or
/// another configuration value. Every other variant is a runtime failure:
/// the network call did not complete, the peer was unreachable, or the
/// response could not be used.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("{0}")]
    Config(String),
    #[error("{0}")]
    Fail(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
}

impl DiscoveryError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn fail(msg: impl Into<String>) -> Self {
        Self::Fail(msg.into())
    }
}

pub type Result<T> = std::result::Result<T, DiscoveryError>;
