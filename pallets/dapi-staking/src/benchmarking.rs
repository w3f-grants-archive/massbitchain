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
	Ledger::<T>::remove_all(None);
	RegisteredProviders::<T>::remove_all(None);
	GeneralEraInfo::<T>::remove_all(None);
	ProviderEraStake::<T>::remove_all(None);
	GeneralStakerInfo::<T>::remove_all(None);
	CurrentEra::<T>::kill();
	BlockRewardAccumulator::<T>::kill();

	// Initialize the first block
	DapiStaking::<T>::on_unbalanced(T::Currency::issue(BLOCK_REWARD.into()));
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
		DapiStaking::<T>::on_unbalanced(T::Currency::issue(BLOCK_REWARD.into()));
		DapiStaking::<T>::on_initialize(System::<T>::block_number());
	}
}

/// Used to register a provider.
fn register_provider<T: Config>() -> Result<(T::AccountId, T::ProviderId), &'static str> {
	let operator: T::AccountId = account("operator", 10000, SEED);
	T::Currency::make_free_balance_be(&operator, BalanceOf::<T>::max_value());
	let provider_id = T::ProviderId::default();
	let deposit = T::RegisterDeposit::get() + T::MinimumStakingAmount::get();
	DapiStaking::<T>::register(operator.clone(), provider_id.clone(), deposit)?;
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
	let stake_balance = T::MinimumStakingAmount::get();
	let mut stakers = Vec::new();

	for id in 0..number_of_stakers {
		let staker_acc: T::AccountId = account("pre_staker", id, seed);
		stakers.push(staker_acc.clone());
		T::Currency::make_free_balance_be(&staker_acc, BalanceOf::<T>::max_value());

		DapiStaking::<T>::stake(
			RawOrigin::Signed(staker_acc).into(),
			provider_id.clone(),
			stake_balance.clone(),
		)?;
	}

	Ok(stakers)
}

benchmarks! {
	withdraw_from_unregistered_staker {
		initialize::<T>();
		let (operator, provider_id) = register_provider::<T>()?;

		let stakers = prepare_stake::<T>(1, &provider_id, SEED)?;
		let staker = stakers[0].clone();
		let stake_amount = BalanceOf::<T>::max_value() / 2u32.into();

		DapiStaking::<T>::stake(RawOrigin::Signed(staker.clone()).into(), provider_id.clone(), stake_amount)?;

		DapiStaking::<T>::unregister(provider_id.clone())?;

		let current_era = DapiStaking::<T>::current_era();
		advance_to_era::<T>(current_era + 1 + T::UnbondingPeriod::get());

	}: _(RawOrigin::Signed(staker.clone()), provider_id.clone())
	verify {
		let staker_info = DapiStaking::<T>::staker_info(&staker, &provider_id);
		assert!(staker_info.latest_staked_value().is_zero());
	}

	withdraw_from_unregistered_operator {
		initialize::<T>();
		let (operator, provider_id) = register_provider::<T>()?;

		DapiStaking::<T>::unregister(provider_id.clone())?;

		let current_era = DapiStaking::<T>::current_era();
		advance_to_era::<T>(current_era + 1 + T::UnbondingPeriod::get());

	}: _(RawOrigin::Signed(operator.clone()), provider_id.clone())
	verify {
		let provider = RegisteredProviders::<T>::get(&provider_id).unwrap();
		assert!(provider.unreserved);
	}

	stake {
		initialize::<T>();

		let (_, provider_id) = register_provider::<T>()?;
		prepare_stake::<T>(T::MaxNumberOfStakersPerProvider::get() - 2, &provider_id, SEED)?;

		let staker = whitelisted_caller();
		let _ = T::Currency::make_free_balance_be(&staker, BalanceOf::<T>::max_value());
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

	}: _(RawOrigin::Signed(staker.clone()), provider_id.clone(), amount)
	verify {
		assert_last_event::<T>(Event::<T>::Stake{staker, provider_id, amount}.into());
	}

	unstake {
		initialize::<T>();

		let (_, provider_id) = register_provider::<T>()?;
		prepare_stake::<T>(T::MaxNumberOfStakersPerProvider::get() - 2, &provider_id, SEED)?;

		let staker = whitelisted_caller();
		let _ = T::Currency::make_free_balance_be(&staker, BalanceOf::<T>::max_value());
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

		DapiStaking::<T>::stake(RawOrigin::Signed(staker.clone()).into(), provider_id.clone(), amount)?;

	}: _(RawOrigin::Signed(staker.clone()), provider_id.clone(), amount)
	verify {
		assert_last_event::<T>(Event::<T>::Unstake{staker, provider_id, amount}.into());
	}

	withdraw_unstaked {
		initialize::<T>();

		let (_, provider_id) = register_provider::<T>()?;
		prepare_stake::<T>(T::MaxNumberOfStakersPerProvider::get() - 2, &provider_id, SEED)?;

		let staker = whitelisted_caller();
		let _ = T::Currency::make_free_balance_be(&staker, BalanceOf::<T>::max_value());
		let stake_amount = BalanceOf::<T>::max_value() / 2u32.into();
		let unstake_amount = stake_amount / 2u32.into();

		DapiStaking::<T>::stake(RawOrigin::Signed(staker.clone()).into(), provider_id.clone(), stake_amount)?;
		DapiStaking::<T>::unstake(RawOrigin::Signed(staker.clone()).into(), provider_id.clone(), unstake_amount)?;

		let current_era = DapiStaking::<T>::current_era();
		advance_to_era::<T>(current_era + 1 + T::UnbondingPeriod::get());

	}: _(RawOrigin::Signed(staker.clone()))
	verify {
		assert_last_event::<T>(Event::<T>::Withdrawn{staker, amount: unstake_amount}.into());
	}

	claim_staker {
		initialize::<T>();
		let (_, provider_id) = register_provider::<T>()?;

		let number_of_stakers = 3;
		let claim_era = DapiStaking::<T>::current_era();
		let stakers = prepare_stake::<T>(number_of_stakers, &provider_id, SEED)?;
		let staker = stakers[0].clone();
		advance_to_era::<T>(claim_era + 1u32);

	}: _(RawOrigin::Signed(staker.clone()), provider_id.clone())
	verify {
		let mut staker_info = DapiStaking::<T>::staker_info(&staker, &provider_id);
		let (era, _) = staker_info.claim();
		assert!(era > claim_era);
	}

	claim_operator {
		initialize::<T>();
		let (operator, provider_id) = register_provider::<T>()?;

		let number_of_stakers = 3;
		let claim_era = DapiStaking::<T>::current_era();
		prepare_stake::<T>(number_of_stakers, &provider_id, SEED)?;
		advance_to_era::<T>(claim_era + 1u32);

	}: _(RawOrigin::Signed(operator.clone()), provider_id.clone(), claim_era)
	verify {
		let staking_info = DapiStaking::<T>::provider_stake_info(&provider_id, claim_era).unwrap();
		assert!(staking_info.provider_reward_claimed);
	}


	force_new_era {
	}: _(RawOrigin::Root)
}

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
