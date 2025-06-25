//! Defines a connection to gitlab
use reqwest::Client;

/// Infos needed to connect to gitlab
#[derive(Clone)]
pub struct Connection {
    /// Hostname
    pub hostname: String,
    /// [`reqwest`] client
    pub http_client: Client,
    /// Authentication token
    pub token: String,
}

impl Connection {
    /// Creates a new [`Connection`]
    pub fn new(
        hostname: String,
        token: String,
        accept_invalid_certs: bool,
    ) -> Result<Self, reqwest::Error> {
        let http_client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(accept_invalid_certs)
            .build()?;
        Ok(Self {
            hostname,
            http_client,
            token,
        })
    }
}
