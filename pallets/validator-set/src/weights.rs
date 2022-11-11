//! Autogenerated weights for pallet_validator_set
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2022-07-05, STEPS: `20`, REPEAT: 10, LOW RANGE: `[]`, HIGH RANGE: `[]`
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
// pallet_validator_set
// --extrinsic
// *
// --steps
// 20
// --repeat
// 10
// --output
// ./pallets/validator-set/src/weights.rs
// --template
// ./benchmarking/frame-weight-template.hbs

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

/// Weight functions needed for pallet_validator_set.
pub trait WeightInfo {
	#[rustfmt::skip]
	fn set_invulnerables(b: u32, ) -> Weight;
	#[rustfmt::skip]
	fn set_desired_candidates() -> Weight;
	#[rustfmt::skip]
	fn set_candidacy_bond() -> Weight;
	#[rustfmt::skip]
	fn register_as_candidate(c: u32, ) -> Weight;
	#[rustfmt::skip]
	fn leave_intent(c: u32, ) -> Weight;
	#[rustfmt::skip]
	fn note_author() -> Weight;
	#[rustfmt::skip]
	fn new_session(r: u32, c: u32, ) -> Weight;
}

/// Weights for pallet_validator_set using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: Session NextKeys (r:1 w:0)
	// Storage: ValidatorSet Invulnerables (r:0 w:1)
	#[rustfmt::skip]
	fn set_invulnerables(b: u32, ) -> Weight {
		(8_432_000 as Weight)
			// Standard Error: 14_000
			.saturating_add((3_302_000 as Weight).saturating_mul(b as Weight))
			.saturating_add(T::DbWeight::get().reads((1 as Weight).saturating_mul(b as Weight)))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: ValidatorSet DesiredCandidates (r:0 w:1)
	#[rustfmt::skip]
	fn set_desired_candidates() -> Weight {
		(8_000_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: ValidatorSet CandidacyBond (r:0 w:1)
	#[rustfmt::skip]
	fn set_candidacy_bond() -> Weight {
		(8_000_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: ValidatorSet Candidates (r:1 w:1)
	// Storage: ValidatorSet DesiredCandidates (r:1 w:0)
	// Storage: ValidatorSet Invulnerables (r:1 w:0)
	// Storage: Session NextKeys (r:1 w:0)
	// Storage: ValidatorSet CandidacyBond (r:1 w:0)
	// Storage: ValidatorSet LastAuthoredBlock (r:0 w:1)
	#[rustfmt::skip]
	fn register_as_candidate(c: u32, ) -> Weight {
		(33_149_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((58_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(5 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: ValidatorSet Candidates (r:1 w:1)
	// Storage: ValidatorSet LastAuthoredBlock (r:0 w:1)
	#[rustfmt::skip]
	fn leave_intent(c: u32, ) -> Weight {
		(22_624_000 as Weight)
			// Standard Error: 0
			.saturating_add((62_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: System Account (r:2 w:2)
	// Storage: System BlockWeight (r:1 w:1)
	// Storage: ValidatorSet LastAuthoredBlock (r:0 w:1)
	#[rustfmt::skip]
	fn note_author() -> Weight {
		(30_000_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	// Storage: ValidatorSet Candidates (r:1 w:1)
	// Storage: ValidatorSet LastAuthoredBlock (r:200 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: ValidatorSet SlashDestination (r:1 w:0)
	// Storage: Balances TotalIssuance (r:1 w:1)
	// Storage: ValidatorSet Invulnerables (r:1 w:0)
	// Storage: System BlockWeight (r:1 w:1)
	#[rustfmt::skip]
	fn new_session(r: u32, c: u32, ) -> Weight {
		(0 as Weight)
			// Standard Error: 1_796_000
			.saturating_add((7_415_000 as Weight).saturating_mul(r as Weight))
			// Standard Error: 1_796_000
			.saturating_add((25_726_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads((2 as Weight).saturating_mul(c as Weight)))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(r as Weight)))
			.saturating_add(T::DbWeight::get().writes((1 as Weight).saturating_mul(c as Weight)))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: Session NextKeys (r:1 w:0)
	// Storage: ValidatorSet Invulnerables (r:0 w:1)
	#[rustfmt::skip]
	fn set_invulnerables(b: u32, ) -> Weight {
		(8_432_000 as Weight)
			// Standard Error: 14_000
			.saturating_add((3_302_000 as Weight).saturating_mul(b as Weight))
			.saturating_add(RocksDbWeight::get().reads((1 as Weight).saturating_mul(b as Weight)))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: ValidatorSet DesiredCandidates (r:0 w:1)
	#[rustfmt::skip]
	fn set_desired_candidates() -> Weight {
		(8_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: ValidatorSet CandidacyBond (r:0 w:1)
	#[rustfmt::skip]
	fn set_candidacy_bond() -> Weight {
		(8_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: ValidatorSet Candidates (r:1 w:1)
	// Storage: ValidatorSet DesiredCandidates (r:1 w:0)
	// Storage: ValidatorSet Invulnerables (r:1 w:0)
	// Storage: Session NextKeys (r:1 w:0)
	// Storage: ValidatorSet CandidacyBond (r:1 w:0)
	// Storage: ValidatorSet LastAuthoredBlock (r:0 w:1)
	#[rustfmt::skip]
	fn register_as_candidate(c: u32, ) -> Weight {
		(33_149_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((58_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(5 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	// Storage: ValidatorSet Candidates (r:1 w:1)
	// Storage: ValidatorSet LastAuthoredBlock (r:0 w:1)
	#[rustfmt::skip]
	fn leave_intent(c: u32, ) -> Weight {
		(22_624_000 as Weight)
			// Standard Error: 0
			.saturating_add((62_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	// Storage: System Account (r:2 w:2)
	// Storage: System BlockWeight (r:1 w:1)
	// Storage: ValidatorSet LastAuthoredBlock (r:0 w:1)
	#[rustfmt::skip]
	fn note_author() -> Weight {
		(30_000_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(3 as Weight))
			.saturating_add(RocksDbWeight::get().writes(4 as Weight))
	}
	// Storage: ValidatorSet Candidates (r:1 w:1)
	// Storage: ValidatorSet LastAuthoredBlock (r:200 w:1)
	// Storage: System Account (r:1 w:1)
	// Storage: ValidatorSet SlashDestination (r:1 w:0)
	// Storage: Balances TotalIssuance (r:1 w:1)
	// Storage: ValidatorSet Invulnerables (r:1 w:0)
	// Storage: System BlockWeight (r:1 w:1)
	#[rustfmt::skip]
	fn new_session(r: u32, c: u32, ) -> Weight {
		(0 as Weight)
			// Standard Error: 1_796_000
			.saturating_add((7_415_000 as Weight).saturating_mul(r as Weight))
			// Standard Error: 1_796_000
			.saturating_add((25_726_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads((2 as Weight).saturating_mul(c as Weight)))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(r as Weight)))
			.saturating_add(RocksDbWeight::get().writes((1 as Weight).saturating_mul(c as Weight)))
	}
}
