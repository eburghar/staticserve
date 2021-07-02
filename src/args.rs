use argh::FromArgs;

#[derive(FromArgs)]
/// Extract latest projects archives from a gitlab server
pub struct Opts {
	#[argh(option, short = 'c', default="\"/etc/staticserve.yaml\".to_owned()")]
	/// configuration file containing projects and gitlab connection parameters
	pub config: String,
	#[argh(switch, short = 'v')]
	/// more detailed output
	pub verbose: bool,
}
