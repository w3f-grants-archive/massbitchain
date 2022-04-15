#![cfg_attr(not(feature = "std"), no_std)]

use super::*;
use frame_support::pallet_prelude::*;

#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct Project<AccountId, ChainId> {
	pub consumer: AccountId,
	pub chain_id: ChainId,
	pub quota: u128,
	pub usage: u128,
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum ProviderType {
	Gateway,
	Node,
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum ProviderState {
	Registered,
	Active,
	InActive,
}

#[derive(Clone, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct Provider<AccountId, ChainId> {
	pub provider_type: ProviderType,
	pub operator: AccountId,
	pub chain_id: ChainId,
	pub state: ProviderState,
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum ProviderDeactivateReason {
	BadPerformance { requests: u64, success_rate: u32, average_latency: u32 },
	OutOfSync,
	UnRegistered,
}
