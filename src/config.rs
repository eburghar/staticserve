use actix_cachecontrol_middleware::data::CacheControl;
use actix_token_middleware::data::Jwt;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::HashMap, fmt, fs::File, path::PathBuf, process::Command};

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

#[derive(Debug, Deserialize, Clone)]
pub struct Hooks {
	/// executed each time the content is updated
	pub updated: Option<Vec<String>>,
}

pub enum HookType {
	Updated,
}

impl fmt::Display for HookType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			HookType::Updated => write!(f, "updated"),
		}
	}
}

impl Hooks {
	pub fn trigger(&self, hook_type: HookType) {
		let hook = match hook_type {
			HookType::Updated => &self.updated,
		};
		if let Some(ref cmds) = hook {
			for cmd_str in cmds {
				let args: Vec<&str> = cmd_str.split_whitespace().collect();
				if !args.is_empty() {
					// enforce absolute exec path for security reason
					if args[0].starts_with('/') {
						let mut cmd = Command::new(&args[0]);
						if args.len() > 1 {
							cmd.args(&args[1..]);
						}
						log::info!("  hook {} trigerred. Executing \"{}\"", hook_type, cmd_str);
						let res = cmd.output();
						if res.is_err() {
							log::error!("Executing \"{}\"", cmd_str);
						}
					} else {
						log::error!(
							"cmd \"{}\" must be absolute and start with / to be executed",
							cmd_str
						);
					}
				}
			}
		}
	}
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
	/// hooks
	pub hooks: Option<Hooks>,
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
