#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{dispatch::DispatchResultWithPostInfo, RuntimeDebug};
use scale_info::TypeInfo;

#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct MassbitId([u8; 36]);

impl Default for MassbitId {
	fn default() -> Self {
		MassbitId([1; 36])
	}
}

pub trait DapiStaking<AccountId, Provider, Balance> {
	fn register_provider(
		origin: AccountId,
		provider_id: Provider,
		deposit: Balance,
	) -> DispatchResultWithPostInfo;

	fn unregister_provider(provider_id: Provider) -> DispatchResultWithPostInfo;
}
