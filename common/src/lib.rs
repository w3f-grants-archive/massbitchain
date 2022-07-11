#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;

#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct MassbitId([u8; 36]);

impl Default for MassbitId {
	fn default() -> Self {
		MassbitId([1; 36])
	}
}

impl MassbitId {
	pub fn repeat_byte(byte: u8) -> Self {
		MassbitId([byte; 36])
	}
}
