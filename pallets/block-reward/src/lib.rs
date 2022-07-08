//! # Block Reward Distribution Pallet

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, Get, Imbalance, OnTimestampSet},
};
use frame_system::{ensure_root, pallet_prelude::*};
use sp_runtime::{traits::CheckedAdd, Perbill};
use sp_std::vec;

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarks;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	pub type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The pallet currency type.
		type Currency: Currency<Self::AccountId>;

		/// Payout rewards handler.
		type BeneficiaryPayout: BeneficiaryPayout<NegativeImbalanceOf<Self>>;

		/// The amount of issuance for each block.
		#[pallet::constant]
		type RewardAmount: Get<BalanceOf<Self>>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn reward_config)]
	pub type RewardConfig<T: Config> = StorageValue<_, DistributionConfig, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Distribution config has been updated.
		DistributionConfigChanged(DistributionConfig),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Sum of all rations must be one whole (100%)
		InvalidDistributionConfig,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub reward_config: DistributionConfig,
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			Self { reward_config: Default::default() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			assert!(self.reward_config.is_valid());
			RewardConfig::<T>::put(self.reward_config.clone())
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the reward distribution config parameters which will be used from next block
		/// reward distribution.
		///
		/// It is mandatory that all components of config sum up to one whole (**100%**),
		/// otherwise an error `InvalidDistributionConfig` will be raised.
		#[pallet::weight(T::WeightInfo::set_config())]
		pub fn set_config(
			origin: OriginFor<T>,
			config: DistributionConfig,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			ensure!(config.is_valid(), Error::<T>::InvalidDistributionConfig);
			RewardConfig::<T>::put(config.clone());

			Self::deposit_event(Event::<T>::DistributionConfigChanged(config));

			Ok(().into())
		}
	}

	impl<Moment, T: Config> OnTimestampSet<Moment> for Pallet<T> {
		fn on_timestamp_set(_: Moment) {
			let inflation = T::Currency::issue(T::RewardAmount::get());
			Self::distribute_rewards(inflation);
		}
	}

	impl<T: Config> Pallet<T> {
		/// Distribute reward between beneficiaries.
		fn distribute_rewards(block_reward: NegativeImbalanceOf<T>) {
			let config = <RewardConfig<T>>::get();

			// Calculate balance which will be deposited for each beneficiary
			let provider_balance = config.providers_percent * block_reward.peek();

			// Prepare imbalances
			let (providers_imbalance, validators_imbalance) = block_reward.split(provider_balance);

			// Payout beneficiaries
			T::BeneficiaryPayout::validators(validators_imbalance);
			T::BeneficiaryPayout::providers(providers_imbalance);
		}
	}
}

/// List of parameters used to calculate reward distribution portions for all the beneficiaries.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct DistributionConfig {
	/// Percentage of rewards that goes to providers
	pub providers_percent: Perbill,
	/// Percentage of rewards that goes to validators
	pub validators_percent: Perbill,
}

impl Default for DistributionConfig {
	fn default() -> Self {
		DistributionConfig {
			providers_percent: Perbill::from_percent(50),
			validators_percent: Perbill::from_percent(50),
		}
	}
}

impl DistributionConfig {
	/// `true` if sum of all percentages is `one whole`, `false` otherwise.
	pub fn is_valid(&self) -> bool {
		let percentages = vec![&self.providers_percent, &self.validators_percent];

		let mut accumulator = Perbill::zero();
		for percentage in percentages {
			let result = accumulator.checked_add(percentage);
			if let Some(result) = result {
				accumulator = result
			} else {
				return false
			}
		}

		Perbill::one() == accumulator
	}
}

/// Defines functions used to payout the beneficiaries of block rewards
pub trait BeneficiaryPayout<Imbalance> {
	/// Payout reward to the validators
	fn validators(reward: Imbalance);

	/// Payout reward to providers
	fn providers(reward: Imbalance);
}
