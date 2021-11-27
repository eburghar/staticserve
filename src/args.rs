use argh::FromArgs;

#[derive(FromArgs)]
/// Static file server with ability to upload content and define dynamic routes
pub struct Opts {
	/// configuration file containing projects and gitlab connection parameters
	#[argh(option, short = 'c', default="\"/etc/staticserve.yaml\".to_owned()")]
	pub config: String,

	/// more detailed output
	#[argh(switch, short = 'v')]
	pub verbose: bool,

	/// addr:port to bind to
	#[argh(option, short = 'a', default="\"0.0.0.0:8080\".to_owned()")]
	pub addr: String,
}
