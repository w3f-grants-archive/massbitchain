#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Pallet as Dapi;

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

	Dapi::<T>::add_chain_id(RawOrigin::Root.into(), "eth.mainnet".into()).unwrap();
}

/// Assert that the last event equals the provided one.
fn assert_last_event<T: Config>(event: <T as Config>::Event) {
	frame_system::Pallet::<T>::assert_last_event(event.into());
}

benchmarks! {
	register_project {
		initialize::<T>();

		let consumer: T::AccountId = account("consumer", 10000, SEED);
		T::Currency::make_free_balance_be(&consumer, BalanceOf::<T>::max_value());

		let project_id = T::MassbitId::default();
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

		let chain_id = "eth.mainnet".into();

	}: _(RawOrigin::Signed(consumer.clone()), project_id.clone(), chain_id, amount.clone())

	deposit_project {
		initialize::<T>();

		let consumer: T::AccountId = account("consumer", 10000, SEED);
		T::Currency::make_free_balance_be(&consumer, BalanceOf::<T>::max_value());

		let project_id = T::MassbitId::default();
		let amount = BalanceOf::<T>::max_value() / 3u32.into();

		let chain_id = "eth.mainnet".into();
		Dapi::<T>::register_project(RawOrigin::Signed(consumer.clone()).into(), project_id.clone(), chain_id, amount.clone())?;

	}: _(RawOrigin::Signed(consumer.clone()), project_id.clone(), amount.clone())

	register_provider {
		initialize::<T>();

		let operator: T::AccountId = account("operator", 10000, SEED);
		T::Currency::make_free_balance_be(&operator, BalanceOf::<T>::max_value());

		let provider_id = T::MassbitId::default();
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

		let chain_id = "eth.mainnet".into();

	}: _(RawOrigin::Signed(operator.clone()), provider_id.clone(), ProviderType::Gateway, chain_id, amount.clone())

	unregister_provider {
		initialize::<T>();

		let operator: T::AccountId = account("operator", 10000, SEED);
		T::Currency::make_free_balance_be(&operator, BalanceOf::<T>::max_value());

		let provider_id = T::MassbitId::default();
		let amount = BalanceOf::<T>::max_value() / 2u32.into();

		let chain_id = "eth.mainnet".into();
		Dapi::<T>::register_provider(RawOrigin::Signed(operator.clone()).into(), provider_id.clone(), ProviderType::Gateway, chain_id, amount.clone())?;

	}: _(RawOrigin::Signed(operator.clone()), provider_id.clone())

	add_chain_id {
		ChainIds::<T>::kill();

	}: _(RawOrigin::Root, "eth.mainnet".into())

	remove_chain_id {
		ChainIds::<T>::kill();

		Dapi::<T>::add_chain_id(RawOrigin::Root.into(), "eth.mainnet".into())?;

	}: _(RawOrigin::Root, "eth.mainnet".into())

	add_regulator {
		initialize::<T>();
		let regulator: T::AccountId = account("regulator", 10000, SEED);

	}: _(RawOrigin::Root, regulator)

	remove_regulator {
		initialize::<T>();

		let regulator: T::AccountId = account("regulator", 10000, SEED);
		Dapi::<T>::add_regulator(RawOrigin::Root.into(), regulator.clone())?;

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
	crate::benchmarking::tests::new_test_ext(),
	crate::mock::TestRuntime,
);
