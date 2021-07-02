use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::File;
use std::path::PathBuf;
use std::collections::HashMap;

/// Config file
#[derive(Deserialize, Clone)]
pub struct Config {
	/// working directory
	pub dir: PathBuf,
	/// root of the web server (relative to dir)
	pub root: PathBuf,
	/// token for uploading
	pub token: String,
	/// dynamic routes pointing to static files
	pub routes: HashMap<String, String>
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

