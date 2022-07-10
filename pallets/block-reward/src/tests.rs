use super::{pallet::Error, Event, *};
use frame_support::{assert_noop, assert_ok};
use mock::*;
use sp_runtime::{
	traits::{AccountIdConversion, BadOrigin, Zero},
	Perbill,
};

#[test]
fn default_distribution_config_is_valid() {
	let config = DistributionConfig::default();
	assert!(config.is_valid());
}

#[test]
fn distribution_config_is_valid() {
	let config = DistributionConfig {
		providers_percent: Perbill::from_percent(100),
		validators_percent: Zero::zero(),
	};
	assert!(config.is_valid());

	let config = DistributionConfig {
		providers_percent: Perbill::from_percent(80),
		validators_percent: Perbill::from_percent(20),
	};
	assert!(config.is_valid());
}

#[test]
fn distribution_config_is_invalid() {
	let config =
		DistributionConfig { providers_percent: Perbill::from_percent(100), ..Default::default() };
	assert!(!config.is_valid());

	let config = DistributionConfig {
		providers_percent: Perbill::from_percent(80),
		validators_percent: Perbill::from_percent(19),
	};
	assert!(!config.is_valid());
}

#[test]
pub fn set_configuration_fail() {
	ExternalityBuilder::build().execute_with(|| {
		assert_noop!(BlockReward::set_config(Origin::signed(1), Default::default()), BadOrigin);

		let config = DistributionConfig {
			providers_percent: Perbill::from_percent(100),
			..Default::default()
		};
		assert!(!config.is_valid());
		assert_noop!(
			BlockReward::set_config(Origin::root(), config),
			Error::<TestRuntime>::InvalidDistributionConfig,
		);
	})
}

#[test]
pub fn set_configuration_success() {
	ExternalityBuilder::build().execute_with(|| {
		let config = DistributionConfig {
			providers_percent: Perbill::from_percent(50),
			validators_percent: Perbill::from_percent(50),
		};
		assert!(config.is_valid());

		assert_ok!(BlockReward::set_config(Origin::root(), config.clone()));
		System::assert_last_event(mock::Event::BlockReward(Event::DistributionConfigChanged(
			config.clone(),
		)));

		assert_eq!(RewardConfig::<TestRuntime>::get(), config);
	})
}

#[test]
pub fn inflation_and_total_issuance_as_expected() {
	ExternalityBuilder::build().execute_with(|| {
		let init_issuance = <TestRuntime as Config>::Currency::total_issuance();
		for block in 0..10 {
			assert_eq!(
				<TestRuntime as Config>::Currency::total_issuance(),
				block * BLOCK_REWARD + init_issuance
			);
			BlockReward::on_timestamp_set(0);
			assert_eq!(
				<TestRuntime as Config>::Currency::total_issuance(),
				(block + 1) * BLOCK_REWARD + init_issuance
			);
		}
	})
}

#[test]
pub fn reward_distribution_as_expected() {
	ExternalityBuilder::build().execute_with(|| {
		let init_balance_snapshot = FreeBalanceSnapshot::new();
		assert!(init_balance_snapshot.is_zero());

		let config = DistributionConfig {
			validators_percent: Perbill::from_percent(20),
			providers_percent: Perbill::from_percent(80),
		};
		assert!(config.is_valid());
		assert_ok!(BlockReward::set_config(Origin::root(), config.clone()));

		for _ in 1..=100 {
			let init_balance_state = FreeBalanceSnapshot::new();
			let rewards = Rewards::calculate(&config);
			BlockReward::on_timestamp_set(0);
			let final_balance_state = FreeBalanceSnapshot::new();
			init_balance_state.assert_distribution(&final_balance_state, &rewards);
		}
	})
}

/// Represents free balance snapshot at a specific point in time
#[derive(PartialEq, Eq, Clone, RuntimeDebug)]
struct FreeBalanceSnapshot {
	validators: Balance,
	providers: Balance,
}

impl FreeBalanceSnapshot {
	/// Creates a new free balance snapshot using current balance state.
	fn new() -> Self {
		Self {
			validators: <TestRuntime as Config>::Currency::free_balance(
				&VALIDATOR_POT.into_account_truncating(),
			),
			providers: <TestRuntime as Config>::Currency::free_balance(
				&PROVIDER_POT.into_account_truncating(),
			),
		}
	}

	/// `true` if all free balances equal `Zero`, `false` otherwise
	fn is_zero(&self) -> bool {
		self.providers.is_zero() && self.providers.is_zero()
	}

	/// Asserts that `post_reward_state` is as expected.
	fn assert_distribution(&self, post_reward_state: &Self, rewards: &Rewards) {
		assert_eq!(self.validators + rewards.validators_reward, post_reward_state.validators);
		assert_eq!(self.providers + rewards.providers_reward, post_reward_state.providers);
	}
}

/// Represents reward distribution balances for a single distribution.
#[derive(PartialEq, Eq, Clone, RuntimeDebug)]
struct Rewards {
	validators_reward: Balance,
	providers_reward: Balance,
}

impl Rewards {
	/// Pre-calculates the reward distribution, using the provided `DistributionConfig`.
	/// Method assumes that total issuance will be increased by `BLOCK_REWARD`.
	fn calculate(config: &DistributionConfig) -> Self {
		let validators_reward = config.validators_percent * BLOCK_REWARD;
		let providers_reward = config.providers_percent * BLOCK_REWARD;
		Self { validators_reward, providers_reward }
	}
}
