//! Autogenerated weights for pallet_dapi_staking
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-07-06, STEPS: `20`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// ./target/release/massbit-node
// benchmark
// pallet
// --chain
// dev
// --execution
// wasm
// --wasm-execution
// compiled
// --pallet
// pallet_dapi_staking
// --extrinsic
// *
// --steps
// 20
// --repeat
// 10
// --output
// ./pallets/dapi-staking/src/weights.rs
// --template
// ./benchmarking/frame-weight-template.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_dapi_staking.
pub trait WeightInfo {
	#[rustfmt::skip]
	fn set_blocks_per_era() -> Weight;
	#[rustfmt::skip]
	fn provider_bond_more() -> Weight;
	#[rustfmt::skip]
	fn provider_bond_less() -> Weight;
	#[rustfmt::skip]
	fn delegate() -> Weight;
	#[rustfmt::skip]
	fn delegator_unstake() -> Weight;
	#[rustfmt::skip]
	fn withdraw_unbonded() -> Weight;
	#[rustfmt::skip]
	fn claim_provider() -> Weight;
	#[rustfmt::skip]
	fn claim_delegator() -> Weight;
	#[rustfmt::skip]
	fn provider_withdraw_unregistered() -> Weight;
	#[rustfmt::skip]
	fn delegator_withdraw_unregistered() -> Weight;
}

/// Weights for pallet_dapi_staking using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: DapiStaking Era (r:1 w:1)
	#[rustfmt::skip]
	fn set_blocks_per_era() -> Weight {
		(10_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn provider_bond_more() -> Weight {
		(27_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking UnbondingInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn provider_bond_less() -> Weight {
		(21_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn delegate() -> Weight {
		(32_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(3 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking UnbondingInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn delegator_unstake() -> Weight {
		(27_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(6 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	// Storage: DapiStaking UnbondingInfo (r:1 w:1)
	// Storage: DapiStaking Era (r:1 w:0)
	#[rustfmt::skip]
	fn withdraw_unbonded() -> Weight {
		(25_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:0)
	#[rustfmt::skip]
	fn claim_provider() -> Weight {
		(20_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:0)
	// Storage: DapiStaking EraState (r:1 w:0)
	#[rustfmt::skip]
	fn claim_delegator() -> Weight {
		(23_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:1)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	#[rustfmt::skip]
	fn provider_withdraw_unregistered() -> Weight {
		(27_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	#[rustfmt::skip]
	fn delegator_withdraw_unregistered() -> Weight {
		(28_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: DapiStaking Era (r:1 w:1)
	#[rustfmt::skip]
	fn set_blocks_per_era() -> Weight {
		(10_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn provider_bond_more() -> Weight {
		(27_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(3 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking UnbondingInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn provider_bond_less() -> Weight {
		(21_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(3 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn delegate() -> Weight {
		(32_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(3 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking UnbondingInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:1)
	#[rustfmt::skip]
	fn delegator_unstake() -> Weight {
		(27_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(6 as Weight))
			.saturating_add(RocksDbWeight::get().writes(4 as Weight))
	}
	// Storage: DapiStaking UnbondingInfo (r:1 w:1)
	// Storage: DapiStaking Era (r:1 w:0)
	#[rustfmt::skip]
	fn withdraw_unbonded() -> Weight {
		(25_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:1)
	// Storage: DapiStaking EraState (r:1 w:0)
	#[rustfmt::skip]
	fn claim_provider() -> Weight {
		(20_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(4 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:0)
	// Storage: DapiStaking EraState (r:1 w:0)
	#[rustfmt::skip]
	fn claim_delegator() -> Weight {
		(23_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:1)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking ProviderEraInfo (r:1 w:0)
	// Storage: System Account (r:1 w:1)
	#[rustfmt::skip]
	fn provider_withdraw_unregistered() -> Weight {
		(27_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(4 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	// Storage: DapiStaking ProviderInfo (r:1 w:0)
	// Storage: DapiStaking Era (r:1 w:0)
	// Storage: DapiStaking DelegationInfo (r:1 w:1)
	// Storage: System Account (r:1 w:1)
	#[rustfmt::skip]
	fn delegator_withdraw_unregistered() -> Weight {
		(28_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(4 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
}