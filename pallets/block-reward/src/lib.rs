//! # Block Reward Distribution Pallet

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, Get, Imbalance, OnTimestampSet},
};
use frame_system::{ensure_root, pallet_prelude::*};
use sp_runtime::{
	traits::{CheckedAdd, Zero},
	Perbill,
};
use sp_std::vec;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	/// The balance type of this pallet.
	pub(crate) type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Negative imbalance type of this pallet.
	pub(crate) type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

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
	}

	#[pallet::storage]
	#[pallet::getter(fn reward_config)]
	pub type RewardDistributionConfigStorage<T: Config> =
		StorageValue<_, RewardDistributionConfig, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Distribution configuration has been updated.
		DistributionConfigurationChanged(RewardDistributionConfig),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Sum of all rations must be one whole (100%)
		InvalidDistributionConfiguration,
	}

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub reward_config: RewardDistributionConfig,
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
			assert!(self.reward_config.is_consistent());
			RewardDistributionConfigStorage::<T>::put(self.reward_config.clone())
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the reward distribution configuration parameters which will be used from next block
		/// reward distribution.
		///
		/// It is mandatory that all components of configuration sum up to one whole (**100%**),
		/// otherwise an error `InvalidDistributionConfiguration` will be raised.
		///
		/// - `reward_distro_params` - reward distribution params
		///
		/// Emits `DistributionConfigurationChanged` with config embedded into event itself.
		#[pallet::weight(100)]
		pub fn set_configuration(
			origin: OriginFor<T>,
			reward_distro_params: RewardDistributionConfig,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			ensure!(
				reward_distro_params.is_consistent(),
				Error::<T>::InvalidDistributionConfiguration
			);
			RewardDistributionConfigStorage::<T>::put(reward_distro_params.clone());

			Self::deposit_event(Event::<T>::DistributionConfigurationChanged(reward_distro_params));

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
		///
		/// # Arguments
		/// * `reward` - reward that will be split and distributed
		fn distribute_rewards(block_reward: NegativeImbalanceOf<T>) {
			let distro_params = Self::reward_config();

			// Calculate balance which will be deposited for each beneficiary
			let provider_balance = distro_params.providers_percent * block_reward.peek();
			let validator_balance = distro_params.validators_percent * block_reward.peek();

			// Prepare imbalances
			let (providers_imbalance, remainder) = block_reward.split(provider_balance);
			let (validators_imbalance, _) = remainder.split(validator_balance);

			// Payout beneficiaries
			T::BeneficiaryPayout::validators(validators_imbalance);
			T::BeneficiaryPayout::providers(providers_imbalance);
		}
	}
}

/// List of configuration parameters used to calculate reward distribution portions for all the
/// beneficiaries.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub struct RewardDistributionConfig {
	/// Percentage of rewards that goes to providers
	pub providers_percent: Perbill,
	/// Percentage of rewards that goes to validators
	pub validators_percent: Perbill,
}

impl Default for RewardDistributionConfig {
	fn default() -> Self {
		RewardDistributionConfig {
			providers_percent: Perbill::from_percent(100),
			validators_percent: Zero::zero(),
		}
	}
}

impl RewardDistributionConfig {
	/// `true` if sum of all percentages is `one whole`, `false` otherwise.
	pub fn is_consistent(&self) -> bool {
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
