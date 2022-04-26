#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet;
use pallet_dapi::DapiStaking;

use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite, whitelisted_caller};
use frame_support::traits::{Get, OnInitialize};
use frame_system::{Pallet as System, RawOrigin};
use sp_runtime::traits::{Bounded, One};
use sp_std::vec::Vec;

const SEED: u32 = 9000;
const BLOCK_REWARD: u32 = 1000;

/// Used to prepare dAPI staking for testing.
/// Resets all existing storage ensuring a clean run for the code that follows.
fn initialize<T: Config>() {
	// Remove everything from storage
	Era::<T>::kill();
	EraState::<T>::remove_all(None);
	RewardAccumulator::<T>::kill();
	ProviderInfo::<T>::remove_all(None);
	ProviderEraInfo::<T>::remove_all(None);
	DelegationInfo::<T>::remove_all(None);
	UnbondingInfo::<T>::remove_all(None);
}

/// Payout block rewards
fn payout_block_rewards<T: Config>() {
	Pallet::<T>::handle_imbalance(T::Currency::issue(BLOCK_REWARD.into()));
}

/// Assert that the last event equals the provided one.
fn assert_last_event<T: Config>(event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(event.into());
}

/// Advance to the specified era, block by block.
fn advance_to_era<T: Config>(n: EraIndex) {
	while Era::<T>::get().current < n {
		Pallet::<T>::on_initialize(System::<T>::block_number());
		System::<T>::set_block_number(System::<T>::block_number() + One::one());
		// This is performed outside of dAPI staking but we expect it before on_initialize
		payout_block_rewards::<T>();
		Pallet::<T>::on_initialize(System::<T>::block_number());
	}
}

/// Used to register a provider.
fn register_provider<T: Config>() -> Result<(T::AccountId, T::ProviderId), &'static str> {
	let owner: T::AccountId = account("owner", 10000, SEED);
	T::Currency::make_free_balance_be(&owner, BalanceOf::<T>::max_value());
	let provider_id = T::ProviderId::default();
	Pallet::<T>::register_provider(owner.clone(), provider_id.clone(), T::MinProviderStake::get())?;
	Ok((owner, provider_id))
}

/// Used to delegate the given provider with the specified amount of delegators. Method will create
/// new delegator accounts using the provided seed.
fn prepare_delegate<T: Config>(
	delegator_count: u32,
	provider_id: &T::ProviderId,
	seed: u32,
) -> Result<Vec<T::AccountId>, &'static str> {
	let stake_balance = T::MinDelegatorStake::get();
	let mut delegators = Vec::new();
	for id in 0..delegator_count {
		let delegator: T::AccountId = account("delegator", id, seed);
		delegators.push(delegator.clone());
		T::Currency::make_free_balance_be(&delegator, BalanceOf::<T>::max_value());
		Pallet::<T>::delegate(
			RawOrigin::Signed(delegator).into(),
			provider_id.clone(),
			stake_balance,
		)?;
	}
	Ok(delegators)
}

benchmarks! {
	set_blocks_per_era {}: _(RawOrigin::Root, 1200u32)
	verify {
		assert_eq!(Pallet::<T>::era().length, 1200u32);
	}

	provider_bond_more {
		initialize::<T>();
		let (owner, provider_id) = register_provider::<T>()?;
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

	}: _(RawOrigin::Signed(owner.clone()), provider_id.clone(), amount)
	verify {
		assert_last_event::<T>(Event::<T>::ProviderBondedMore{provider_id, amount}.into());
	}

	provider_bond_less {
		initialize::<T>();
		let (owner, provider_id) = register_provider::<T>()?;
		let amount = BalanceOf::<T>::max_value() / 2u32.into();
		Pallet::<T>::provider_bond_more(RawOrigin::Signed(owner.clone()).into(), provider_id.clone(), amount)?;

	}: _(RawOrigin::Signed(owner.clone()), provider_id.clone(), amount)
	verify {
		assert_last_event::<T>(Event::<T>::ProviderBondedLess{provider_id, amount}.into());
	}

	delegate {
		initialize::<T>();

		let (_, provider_id) = register_provider::<T>()?;
		prepare_delegate::<T>(T::MaxDelegatorsPerProvider::get() - 1, &provider_id, SEED)?;

		let delegator = whitelisted_caller();
		let _ = T::Currency::make_free_balance_be(&delegator, BalanceOf::<T>::max_value());
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

	}: _(RawOrigin::Signed(delegator.clone()), provider_id.clone(), amount)
	verify {
		assert_last_event::<T>(Event::<T>::Delegated{delegator, provider_id, amount}.into());
	}

	delegator_unstake {
		initialize::<T>();

		let (_, provider_id) = register_provider::<T>()?;
		prepare_delegate::<T>(T::MaxDelegatorsPerProvider::get() - 1, &provider_id, SEED)?;

		let delegator = whitelisted_caller();
		let _ = T::Currency::make_free_balance_be(&delegator, BalanceOf::<T>::max_value());
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

		Pallet::<T>::delegate(RawOrigin::Signed(delegator.clone()).into(), provider_id.clone(), amount)?;

	}: _(RawOrigin::Signed(delegator.clone()), provider_id.clone(), amount)
	verify {
		assert_last_event::<T>(Event::<T>::DelegatorUnstaked{delegator, provider_id, amount}.into());
	}

	withdraw_unbonded {
		initialize::<T>();

		let (_, provider_id) = register_provider::<T>()?;
		prepare_delegate::<T>(T::MaxDelegatorsPerProvider::get() - 1, &provider_id, SEED)?;

		let delegator = whitelisted_caller();
		let _ = T::Currency::make_free_balance_be(&delegator, BalanceOf::<T>::max_value());
		let stake_amount = BalanceOf::<T>::max_value() / 2u32.into();
		let unstake_amount = stake_amount / 2u32.into();

		Pallet::<T>::delegate(RawOrigin::Signed(delegator.clone()).into(), provider_id.clone(), stake_amount)?;
		Pallet::<T>::delegator_unstake(RawOrigin::Signed(delegator.clone()).into(), provider_id.clone(), unstake_amount)?;

		let current_era = <Era<T>>::get().current;
		advance_to_era::<T>(current_era + 1 + T::UnbondingPeriod::get());

	}: _(RawOrigin::Signed(delegator.clone()))
	verify {
		assert_last_event::<T>(Event::<T>::Withdrawn{who: delegator, amount: unstake_amount}.into());
	}

	claim_provider {
		initialize::<T>();

		let (owner, provider_id) = register_provider::<T>()?;
		let claim_era = <Era<T>>::get().current;
		advance_to_era::<T>(claim_era + 1u32);

	}: _(RawOrigin::Signed(owner.clone()), provider_id.clone(), claim_era)
	verify {
		let info = <ProviderEraInfo<T>>::get(&provider_id, claim_era).unwrap();
		assert!(info.provider_reward_claimed);
	}

	claim_delegator {
		initialize::<T>();
		let (_, provider_id) = register_provider::<T>()?;

		let delegator_count = 3;
		let claim_era = <Era<T>>::get().current;
		let delegators = prepare_delegate::<T>(delegator_count, &provider_id, SEED)?;
		let delegator = delegators[0].clone();
		advance_to_era::<T>(claim_era + 1u32);

	}: _(RawOrigin::Signed(delegator.clone()), provider_id.clone())
	verify {
		let mut delegation = <DelegationInfo<T>>::get(&delegator, &provider_id);
		let (era, _) = delegation.claim();
		assert!(era > claim_era);
	}

	provider_withdraw_unregistered {
		initialize::<T>();
		let (owner, provider_id) = register_provider::<T>()?;

		Pallet::<T>::unregister_provider(provider_id.clone())?;
		let current_era = <Era<T>>::get().current;
		advance_to_era::<T>(current_era + 1 + T::UnbondingPeriod::get());

	}: _(RawOrigin::Signed(owner.clone()), provider_id.clone())
	verify {
		let provider = <ProviderInfo<T>>::get(&provider_id).unwrap();
		assert!(provider.bond_withdrawn);
		assert_last_event::<T>(Event::<T>::Withdrawn{who: owner, amount: T::MinProviderStake::get()}.into());
	}

	delegator_withdraw_unregistered {
		initialize::<T>();
		let (_, provider_id) = register_provider::<T>()?;
		let delegators = prepare_delegate::<T>(1, &provider_id, SEED)?;
		let delegator = delegators[0].clone();

		Pallet::<T>::unregister_provider(provider_id.clone())?;
		let current_era = <Era<T>>::get().current;
		advance_to_era::<T>(current_era + 1 + T::UnbondingPeriod::get());

	}: _(RawOrigin::Signed(delegator.clone()), provider_id.clone())
	verify {
		let delegation = <DelegationInfo<T>>::get(&delegator, &provider_id);
		 assert!(delegation.latest_staked_value().is_zero());
		assert_last_event::<T>(Event::<T>::Withdrawn{who: delegator, amount: T::MinDelegatorStake::get()}.into());
	}
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
	crate::benchmarks::tests::new_test_ext(),
	crate::mock::TestRuntime,
);
