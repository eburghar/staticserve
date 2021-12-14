use actix_cachecontrol_middleware::data::CacheControl;
use actix_token_middleware::data::Jwt;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

#[derive(Deserialize, Clone)]
/// Tls configuration
pub struct Tls {
	/// crt path
	pub crt: PathBuf,
	/// key path
	pub key: PathBuf,
}

fn default_status() -> u16 {
	200
}

#[derive(Deserialize, Clone)]
/// Default page configutation
pub struct DefaultPage {
	/// default file relative to root
	pub file: PathBuf,
	/// default status
	#[serde(default = "default_status")]
	pub status: u16,
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
	/// jwt configuration
	pub jwt: Option<Jwt>,
	/// cache control configuration
	pub cache: Option<CacheControl>,
	/// default page
	pub default: Option<DefaultPage>,
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
