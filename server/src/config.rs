use color_eyre::Result;
use serde::Deserialize;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

#[derive(Deserialize)]
pub struct Config {
    pub frontend: Option<PathBuf>,
    pub region: Option<String>,
    pub key: Option<String>,
    pub listen_address: SocketAddr,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        toml::de::from_str(&content).map_err(Into::into)
    }
}
