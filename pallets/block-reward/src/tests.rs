use super::{pallet::Error, Event, *};
use frame_support::{assert_noop, assert_ok};
use mock::*;
use sp_runtime::{traits::BadOrigin, Perbill};

#[test]
fn default_reward_distribution_config_is_consistent() {
	let reward_config = DistributionConfig::default();
	assert!(reward_config.is_valid());
}

#[test]
pub fn set_configuration_fails() {
	ExternalityBuilder::build().execute_with(|| {
		assert_noop!(BlockReward::set_config(Origin::signed(1), Default::default()), BadOrigin);

		let reward_config = DistributionConfig {
			providers_percent: Perbill::from_percent(100),
			..Default::default()
		};
		assert!(!reward_config.is_valid());
		assert_noop!(
			BlockReward::set_config(Origin::root(), reward_config),
			Error::<TestRuntime>::InvalidDistributionConfig,
		);
	})
}

#[test]
pub fn set_configuration_is_ok() {
	ExternalityBuilder::build().execute_with(|| {
		let reward_config = DistributionConfig {
			providers_percent: Perbill::from_percent(50),
			validators_percent: Perbill::from_percent(50),
		};
		assert!(reward_config.is_valid());

		assert_ok!(BlockReward::set_config(Origin::root(), reward_config.clone()));
		System::assert_last_event(mock::Event::BlockReward(Event::DistributionConfigChanged(
			reward_config.clone(),
		)));

		assert_eq!(RewardConfig::<TestRuntime>::get(), reward_config);
	})
}
