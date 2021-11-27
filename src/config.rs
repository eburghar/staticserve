use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
/// Control Cache behavior
pub struct CacheControl {
	// cache control instructions for paths matching a list prefix
	pub prefixes: Option<BTreeMap<String, String>>,
	// cache control instructions for paths matching a list suffix
	pub suffixes: Option<BTreeMap<String, String>>,
}

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
	pub routes: HashMap<String, String>,
	// jwks endpoint
	pub jwks: String,
	/// claims
	pub claims: BTreeMap<String, String>,
	// cache control configuration
	pub cache: CacheControl,
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
