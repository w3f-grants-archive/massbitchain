#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as DapiStaking;

use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::{Get, OnInitialize, OnUnbalanced};
use frame_system::{Pallet as System, RawOrigin};
use sp_runtime::traits::{Bounded, One};

const SEED: u32 = 9000;
const BLOCK_REWARD: u32 = 1000;

/// Used to prepare Dapi staking for testing.
/// Resets all existing storage ensuring a clean run for the code that follows.
///
/// Also initializes the first block which should start a new era.
fn initialize<T: Config>() {
	// Remove everything from storage
	UnbondingInfo::<T>::remove_all(None);
	ProviderInfo::<T>::remove_all(None);
	EraState::<T>::remove_all(None);
	ProviderEraInfo::<T>::remove_all(None);
	DelegationInfo::<T>::remove_all(None);
	CurrentEra::<T>::kill();
	RewardAccumulator::<T>::kill();

	// Initialize the first block
	DapiStaking::<T>::on_initialize(1u32.into());
}

/// Assert that the last event equals the provided one.
fn assert_last_event<T: Config>(event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(event.into());
}

/// Advance to the specified era, block by block.
fn advance_to_era<T: Config>(n: EraIndex) {
	while DapiStaking::<T>::current_era() < n {
		DapiStaking::<T>::on_initialize(System::<T>::block_number());
		System::<T>::set_block_number(System::<T>::block_number() + One::one());
		// This is performed outside of dapi staking but we expect it before on_initialize
		DapiStaking::<T>::on_initialize(System::<T>::block_number());
	}
}

/// Used to register a provider.
fn register_provider<T: Config>() -> Result<(T::AccountId, T::ProviderId), &'static str> {
	let operator: T::AccountId = account("operator", 10000, SEED);
	T::Currency::make_free_balance_be(&operator, BalanceOf::<T>::max_value());
	let provider_id = T::ProviderId::default();
	let deposit = T::MinProviderStake::get() + T::MinDelegatorStake::get();
	// DapiStaking::<T>::register_provider(operator.clone(), provider_id.clone(), deposit)?;
	Ok((operator, provider_id))
}

/// Used to stake the given provider with the specified amount of stakers.
/// Method will create new staker accounts using the provided seed.
///
/// Returns all created staker accounts in a vector.
fn prepare_stake<T: Config>(
	number_of_stakers: u32,
	provider_id: &T::ProviderId,
	seed: u32,
) -> Result<Vec<T::AccountId>, &'static str> {
	let stake_balance = T::MinDelegatorStake::get();
	let mut stakers = Vec::new();

	for id in 0..number_of_stakers {
		let staker_acc: T::AccountId = account("pre_staker", id, seed);
		stakers.push(staker_acc.clone());
		T::Currency::make_free_balance_be(&staker_acc, BalanceOf::<T>::max_value());

		DapiStaking::<T>::delegate(
			RawOrigin::Signed(staker_acc).into(),
			provider_id.clone(),
			stake_balance.clone(),
		)?;
	}

	Ok(stakers)
}

benchmarks! {}

#[cfg(test)]
mod tests {
	use crate::mock;
	use sp_io::TestExternalities;

	pub fn new_test_ext() -> TestExternalities {
		mock::ExternalityBuilder::build()
	}
}

impl_benchmark_test_suite!(
	DapiStaking,
	crate::benchmarking::tests::new_test_ext(),
	crate::mock::TestRuntime,
);
