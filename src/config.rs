use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use actix_cachecontrol_middleware::data::CacheControl;
use actix_token_middleware::data::Jwt;

#[derive(Deserialize, Clone)]
/// Tls configuration
pub struct Tls {
	/// crt path
	pub crt: PathBuf,
	/// key path
	pub key: PathBuf,
}

/// Config file
#[derive(Deserialize, Clone)]
pub struct Config {
	/// working directory
	pub dir: PathBuf,
	/// root of the web server (relative to dir)
	pub root: PathBuf,
	/// use tls
	pub tls: Option<Tls>,
	/// dynamic routes pointing to static files
	pub routes: Option<HashMap<String, String>>,
	/// jwks endpoint
	pub jwt: Option<Jwt>,
	/// cache control configuration
	pub cache: Option<CacheControl>,
}

impl Config {
	pub fn read(config: &str) -> Result<Config> {
		// open configuration file
		let file = File::open(&config).with_context(|| format!("Can't open {}", &config))?;
		// deserialize configuration
		let config: Config =
			serde_yaml::from_reader(file).with_context(|| format!("Can't read {}", &config))?;
		Ok(config)
	}
}
