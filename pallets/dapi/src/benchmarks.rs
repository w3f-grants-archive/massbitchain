#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet;

use frame_benchmarking::{account, benchmarks, impl_benchmark_test_suite};
use frame_system::RawOrigin;
use sp_runtime::traits::Bounded;

const SEED: u32 = 9000;

fn initialize<T: Config>() {
	// Remove everything from storage
	Projects::<T>::remove_all(None);
	Providers::<T>::remove_all(None);
	Regulators::<T>::kill();
	ChainIds::<T>::kill();

	Pallet::<T>::add_chain_id(RawOrigin::Root.into(), "eth.mainnet".into()).unwrap();
}

/// Assert that the last event equals the provided one.
fn assert_last_event<T: Config>(event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(event.into());
}

benchmarks! {
	register_project {
		initialize::<T>();

		let consumer: T::AccountId = account("consumer", 10000, SEED);
		let _ = T::Currency::make_free_balance_be(&consumer, BalanceOf::<T>::max_value());
		let project_id = T::MassbitId::default();
		let amount = BalanceOf::<T>::max_value() / 2u32.into();
		let quota = Pallet::<T>::calculate_quota(amount);
		let chain_id: Vec<u8> = "eth.mainnet".into();

	}: _(RawOrigin::Signed(consumer.clone()), project_id.clone(), chain_id.clone(), amount.clone())
	verify {
		assert_last_event::<T>(Event::<T>::ProjectRegistered{project_id, consumer, chain_id, quota}.into());
	}

	deposit_project {
		initialize::<T>();

		let consumer: T::AccountId = account("consumer", 10000, SEED);
		let _ = T::Currency::make_free_balance_be(&consumer, BalanceOf::<T>::max_value());
		let project_id = T::MassbitId::default();
		let amount = BalanceOf::<T>::max_value() / 3u32.into();
		let chain_id = "eth.mainnet".into();
		Pallet::<T>::register_project(RawOrigin::Signed(consumer.clone()).into(), project_id.clone(), chain_id, amount.clone())?;

	}: _(RawOrigin::Signed(consumer.clone()), project_id.clone(), amount.clone())

	deposit_provider {
		initialize::<T>();
		let regulator: T::AccountId = account("regulator", 10000, SEED);
		Pallet::<T>::add_regulator(RawOrigin::Root.into(), regulator.clone())?;

		let owner: T::AccountId = account("owner", 20000, SEED);
		let _ = T::Currency::make_free_balance_be(&owner, BalanceOf::<T>::max_value());
		let chain_id: Vec<u8> = "eth.mainnet".into();

		let provider_id = T::MassbitId::default();
		let provider_type = ProviderType::Gateway;
		Pallet::<T>::register_provider(
			RawOrigin::Signed(regulator.clone()).into(),
			provider_id.clone(),
			provider_type,
			owner.clone(),
			chain_id.clone(),
		)?;

		let deposit_amount = BalanceOf::<T>::max_value() / 2u32.into();

	}: _(RawOrigin::Signed(owner.clone()), provider_id.clone(), deposit_amount)
	verify {
		assert_last_event::<T>(Event::<T>::ProviderActivated{provider_id, provider_type}.into());
	}

	unregister_provider {
		initialize::<T>();
		let regulator: T::AccountId = account("regulator", 10000, SEED);
		Pallet::<T>::add_regulator(RawOrigin::Root.into(), regulator.clone())?;

		let owner: T::AccountId = account("owner", 20000, SEED);
		let _ = T::Currency::make_free_balance_be(&owner, BalanceOf::<T>::max_value());
		let chain_id: Vec<u8> = "eth.mainnet".into();

		let provider_id = T::MassbitId::default();
		let provider_type = ProviderType::Gateway;
		Pallet::<T>::register_provider(
			RawOrigin::Signed(regulator.clone()).into(),
			provider_id.clone(),
			provider_type,
			owner.clone(),
			chain_id.clone(),
		)?;

		let deposit_amount = BalanceOf::<T>::max_value() / 2u32.into();
		Pallet::<T>::deposit_provider(RawOrigin::Signed(owner.clone()).into(), provider_id.clone(), deposit_amount)?;

	}: _(RawOrigin::Signed(owner.clone()), provider_id.clone())
	verify {
		assert_last_event::<T>(Event::<T>::ProviderDeactivated{provider_id, provider_type, reason: ProviderDeactivateReason::UnRegistered}.into());
	}

	add_chain_id {
		ChainIds::<T>::kill();
	}: _(RawOrigin::Root, "eth.mainnet".into())

	remove_chain_id {
		ChainIds::<T>::kill();
		Pallet::<T>::add_chain_id(RawOrigin::Root.into(), "eth.mainnet".into())?;
	}: _(RawOrigin::Root, "eth.mainnet".into())

	add_regulator {
		initialize::<T>();
		let regulator: T::AccountId = account("regulator", 10000, SEED);
	}: _(RawOrigin::Root, regulator)

	remove_regulator {
		initialize::<T>();
		let regulator: T::AccountId = account("regulator", 10000, SEED);
		Pallet::<T>::add_regulator(RawOrigin::Root.into(), regulator.clone())?;
	}: _(RawOrigin::Root, regulator)
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
	Dapi,
	crate::benchmarks::tests::new_test_ext(),
	crate::mock::TestRuntime,
);
