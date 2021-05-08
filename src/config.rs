use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs::File;
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Deserialize, Clone)]
pub struct Config {
	pub serve_from: PathBuf,
	pub upload_to: PathBuf,
	pub token: String,
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

