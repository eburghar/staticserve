use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::File;
use std::path::PathBuf;
use std::collections::{BTreeMap, HashMap};

/// Config file
#[derive(Deserialize, Clone)]
pub struct Config {
	/// working directory
	pub dir: PathBuf,
	/// root of the web server (relative to dir)
	pub root: PathBuf,
	/// use tls
	pub tls: bool,
	/// crt path
	pub crt: Option<PathBuf>,
	/// key path
    pub key: Option<PathBuf>,
	/// dynamic routes pointing to static files
	pub routes: HashMap<String, String>,
	/// jwks endpoint
	pub jwks: String,
	/// claims
	pub claims: BTreeMap<String, String>
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

