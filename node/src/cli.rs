use sc_cli::{KeySubcommand, RunCmd, SignCmd, VanityCmd, VerifyCmd};

#[derive(Debug, clap::Parser)]
pub struct Cli {
	#[clap(subcommand)]
	pub subcommand: Option<Subcommand>,

	#[clap(flatten)]
	pub run: RunCmd,
}

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
	/// Build a chain specification.
	BuildSpec(sc_cli::BuildSpecCmd),

	/// Validate blocks.
	CheckBlock(sc_cli::CheckBlockCmd),

	/// Key management cli utilities
	#[clap(subcommand)]
	Key(KeySubcommand),

	/// Sign a message, with a given (secret) key.
	Sign(SignCmd),

	/// Verify a signature for a message, provided on STDIN, with a given (public or secret) key.
	Verify(VerifyCmd),

	/// Generate a seed that provides a vanity address.
	Vanity(VanityCmd),

	/// The custom benchmark subcommand benchmarking runtime pallets.
	#[cfg(feature = "runtime-benchmarks")]
	#[clap(name = "benchmark", about = "Benchmark runtime pallets.")]
	#[clap(subcommand)]
	Benchmark(frame_benchmarking_cli::BenchmarkCmd),
}
