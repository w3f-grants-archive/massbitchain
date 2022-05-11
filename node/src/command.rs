use crate::{
	chain_spec,
	cli::{Cli, Subcommand},
};
use sc_cli::{ChainSpec, RuntimeVersion, SubstrateCli};
use sc_service::PartialComponents;

use super::service::{self, dev, testnet};

trait IdentifyChain {
	fn is_dev(&self) -> bool;
	fn is_testnet(&self) -> bool;
}

impl IdentifyChain for dyn sc_service::ChainSpec {
	fn is_dev(&self) -> bool {
		self.id().starts_with("dev")
	}
	fn is_testnet(&self) -> bool {
		self.id().starts_with("testnet")
	}
}

impl<T: sc_service::ChainSpec + 'static> IdentifyChain for T {
	fn is_dev(&self) -> bool {
		<dyn sc_service::ChainSpec>::is_dev(self)
	}
	fn is_testnet(&self) -> bool {
		<dyn sc_service::ChainSpec>::is_testnet(self)
	}
}

fn load_spec(id: &str) -> std::result::Result<Box<dyn sc_service::ChainSpec>, String> {
	Ok(match id {
		"dev" => Box::new(chain_spec::local::development_config()),
		"testnet-dev" => Box::new(chain_spec::testnet::get_chain_spec()),
		"testnet" => Box::new(chain_spec::TestnetChainSpec::from_json_bytes(
			&include_bytes!("../res/testnet.raw.json")[..],
		)?),
		path =>
			Box::new(chain_spec::TestnetChainSpec::from_json_file(std::path::PathBuf::from(path))?),
	})
}

impl SubstrateCli for Cli {
	fn impl_name() -> String {
		"MassbitChain Node".into()
	}

	fn impl_version() -> String {
		env!("SUBSTRATE_CLI_IMPL_VERSION").into()
	}

	fn description() -> String {
		env!("CARGO_PKG_DESCRIPTION").into()
	}

	fn author() -> String {
		env!("CARGO_PKG_AUTHORS").into()
	}

	fn support_url() -> String {
		"https://github.com/massbitprotocol/massbitchain/issue/new".into()
	}

	fn copyright_start_year() -> i32 {
		2022
	}

	fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
		load_spec(id)
	}

	fn native_runtime_version(chain_spec: &Box<dyn ChainSpec>) -> &'static RuntimeVersion {
		if chain_spec.is_dev() {
			&local_runtime::VERSION
		} else {
			&testnet_runtime::VERSION
		}
	}
}

/// Parse and run command line arguments
pub fn run() -> sc_cli::Result<()> {
	let cli = Cli::from_args();
	match &cli.subcommand {
		Some(Subcommand::BuildSpec(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
		},
		Some(Subcommand::CheckBlock(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			if runner.config().chain_spec.is_testnet() {
				runner.async_run(|config| {
					let PartialComponents { client, task_manager, import_queue, .. } =
						service::new_partial::<testnet::RuntimeApi, testnet::Executor>(&config)?;
					Ok((cmd.run(client, import_queue), task_manager))
				})
			} else {
				runner.async_run(|config| {
					let PartialComponents { client, task_manager, import_queue, .. } =
						service::new_partial::<dev::RuntimeApi, dev::Executor>(&config)?;
					Ok((cmd.run(client, import_queue), task_manager))
				})
			}
		},
		Some(Subcommand::Key(cmd)) => cmd.run(&cli),
		Some(Subcommand::Sign(cmd)) => cmd.run(),
		Some(Subcommand::Verify(cmd)) => cmd.run(),
		Some(Subcommand::Vanity(cmd)) => cmd.run(),
		#[cfg(feature = "frame-benchmarking")]
		Some(Subcommand::Benchmark(cmd)) => {
			let runner = cli.create_runner(cmd)?;
			let chain_spec = &runner.config().chain_spec;
			if chain_spec.is_testnet() {
				runner
					.sync_run(|config| cmd.run::<testnet_runtime::Block, testnet::Executor>(config))
			} else {
				runner.sync_run(|config| cmd.run::<local_runtime::Block, dev::Executor>(config))
			}
		},
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(|config| async move {
				if config.chain_spec.is_testnet() {
					service::start_testnet_node(config).map_err(sc_cli::Error::Service)
				} else {
					service::start_dev_node(config).map_err(sc_cli::Error::Service)
				}
			})
		},
	}
}
