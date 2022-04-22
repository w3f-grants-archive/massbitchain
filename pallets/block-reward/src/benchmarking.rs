#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{benchmarks, impl_benchmark_test_suite};
use frame_system::{Pallet as System, RawOrigin};

/// Assert that the last event equals the provided one.
fn assert_last_event<T: Config>(event: <T as Config>::Event) {
	System::<T>::assert_last_event(event.into());
}

benchmarks! {
	set_config {
		let reward_config = DistributionConfig::default();
		assert!(reward_config.is_valid());
	}: _(RawOrigin::Root, reward_config.clone())
	verify {
		assert_last_event::<T>(Event::<T>::DistributionConfigChanged(reward_config).into());
	}
}

#[cfg(test)]
mod tests {
	use crate::mock;
	use frame_support::sp_io::TestExternalities;

	pub fn new_test_ext() -> TestExternalities {
		mock::ExternalityBuilder::build()
	}
}

impl_benchmark_test_suite!(
	Pallet,
	crate::benchmarking::tests::new_test_ext(),
	crate::mock::TestRuntime,
);
