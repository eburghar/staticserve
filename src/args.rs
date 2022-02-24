use argh::{FromArgs, TopLevelCommand};
use std::path::Path;

#[derive(FromArgs)]
/// Static file server with ability to upload content and define dynamic routes
pub struct Opts {
	/// configuration file containing projects and gitlab connection parameters
	#[argh(option, short = 'c', default = "\"/etc/staticserve.yaml\".to_owned()")]
	pub config: String,

	/// more detailed output
	#[argh(switch, short = 'v')]
	pub verbose: bool,

	/// addr:port to bind to
	#[argh(option, short = 'a', default = "\"0.0.0.0:8080\".to_owned()")]
	pub addr: String,
}

/// copy of argh::from_env to insert command name and version in help text
pub fn from_env<T: TopLevelCommand>() -> T {
	let args: Vec<String> = std::env::args().collect();
	let cmd = Path::new(&args[0])
		.file_name()
		.and_then(|s| s.to_str())
		.unwrap_or(&args[0]);
	let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
	T::from_args(&[cmd], &args_str[1..]).unwrap_or_else(|early_exit| {
		println!("{} {}\n", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
		println!("{}", early_exit.output);
		std::process::exit(match early_exit.status {
			Ok(()) => 0,
			Err(()) => 1,
		})
	})
}
