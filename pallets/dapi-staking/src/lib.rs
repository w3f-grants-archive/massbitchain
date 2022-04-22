//! dAPI Staking Pallet

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	ensure,
	traits::{Currency, ExistenceRequirement, Get, Imbalance, ReservableCurrency, WithdrawReasons},
	weights::Weight,
	PalletId,
};
use frame_system::ensure_signed;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, Saturating, Zero},
	ArithmeticError, Perbill,
};
use sp_std::convert::From;

use pallet_dapi::DapiStaking;

pub mod types;
pub mod weights;

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarking;
#[cfg(test)]
mod mock;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	pub(crate) type EraIndex = u32;
	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	pub type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The balance type of pallet.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// Provider Id type.
		type ProviderId: Parameter + Member + Default;

		/// Percentage of rewards paid to provider.
		#[pallet::constant]
		type ProviderRewardsPercentage: Get<Perbill>;

		/// Minimum stake required to be a provider.
		#[pallet::constant]
		type MinProviderStake: Get<BalanceOf<Self>>;

		/// Maximum number of unique delegators per provider.
		#[pallet::constant]
		type MaxDelegatorsPerProvider: Get<u32>;

		/// Minimum stake required to be a delegator.
		#[pallet::constant]
		type MinDelegatorStake: Get<BalanceOf<Self>>;

		/// Max number of unique `EraStake` values that can exist for a `(delegator, provider)`
		/// pairing. When delegators claims rewards, they will either keep the number of
		/// `EraStake` values the same or they will reduce them by one. Delegators cannot add
		/// an additional `EraStake` value by calling `delegate` or `delegator_unstake` if
		/// they've reached the max number of values.
		///
		/// This ensures that history doesn't grow indefinitely - if there are too many chunks,
		/// delegators should first claim their former rewards before adding additional
		/// `EraStake` values.
		#[pallet::constant]
		type MaxEraStakeValues: Get<u32>;

		/// Number of eras that need to pass until unbonded value can be withdrawn.
		#[pallet::constant]
		type UnbondingPeriod: Get<u32>;

		/// Max number of unlocking chunks per account. If value is zero, unbonding becomes
		/// impossible.
		#[pallet::constant]
		type MaxUnlockingChunks: Get<u32>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// dAPI staking pallet Id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	/// Current era index and next era scheduled transition
	pub type CurrentEra<T: Config> = StorageValue<_, EraInfo<T::BlockNumber>, ValueQuery>;

	/// Total staked & rewarded for a particular era
	#[pallet::storage]
	#[pallet::getter(fn era_state)]
	pub type EraState<T: Config> = StorageMap<_, Twox64Concat, EraIndex, EraMetadata<BalanceOf<T>>>;

	/// Accumulator for rewards (block rewards + project payment) during an era. It is reset at
	/// every new era
	#[pallet::storage]
	#[pallet::getter(fn reward_accumulator)]
	pub type RewardAccumulator<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Registered provider information
	#[pallet::storage]
	#[pallet::getter(fn provider_info)]
	pub(crate) type ProviderInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ProviderId, ProviderMetadata<T::AccountId>>;

	/// Provider state at each era
	#[pallet::storage]
	#[pallet::getter(fn provider_era_info)]
	pub type ProviderEraInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::ProviderId,
		Twox64Concat,
		EraIndex,
		ProviderEraMetadata<BalanceOf<T>>,
	>;

	/// Delegation information of delegator
	#[pallet::storage]
	#[pallet::getter(fn delegation_info)]
	pub(crate) type DelegationInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProviderId,
		Delegation<BalanceOf<T>>,
		ValueQuery,
	>;

	/// Unbonding information of an account
	#[pallet::storage]
	#[pallet::getter(fn unbonding_info)]
	pub type UnbondingInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, UnbondingMetadata<BalanceOf<T>>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Provider has increased a self bond.
		ProviderBondedMore { provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Provider has decreased a self bond.
		ProviderBondedLess { provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Delegator staked funds on a provider.
		Delegation { delegator: T::AccountId, provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Delegator unstaked funds on a provider.
		DelegatorUnstaked {
			delegator: T::AccountId,
			provider_id: T::ProviderId,
			amount: BalanceOf<T>,
		},
		/// Account has withdrawn unbonded funds.
		Withdrawn { who: T::AccountId, amount: BalanceOf<T> },
		/// New staking era. Distribute era rewards to providers.
		NewEra { era: EraIndex, starting_block: T::BlockNumber },
		/// Payout to provider or delegator.
		Payout {
			who: T::AccountId,
			provider_id: T::ProviderId,
			era: EraIndex,
			amount: BalanceOf<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		ProviderDNE,
		InsufficientBond,
		StakingWithNoValue,
		MaxNumberOfStakersExceeded,
		NotOperatedProvider,
		NotStakedProvider,
		UnknownEraReward,
		UnexpectedDelegationInfoEra,
		TooManyEraDelegationValues,
		UnclaimedRewardsRemaining,
		UnstakingWithNoValue,
		TooManyUnlockingChunks,
		NothingToWithdraw,
		EraOutOfBounds,
		NotOwnedProvider,
		AlreadyClaimedInThisEra,
		NotUnregisteredProvider,
		ProviderExists,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let mut era = <CurrentEra<T>>::get();
			if era.should_update(n) {
				let previous_era = era.current;
				era.update(n);
				<CurrentEra<T>>::put(era);

				Self::era_rewards_snapshot(previous_era);
				let consumed_weight = Self::rotate_provider_era_info(previous_era);

				Self::deposit_event(Event::<T>::NewEra {
					era: era.current,
					starting_block: era.starting_block,
				});

				consumed_weight + T::DbWeight::get().reads_writes(2, 2)
			} else {
				T::DbWeight::get().reads(1)
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(100)]
		pub fn provider_bond_more(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(amount > Zero::zero(), Error::<T>::StakingWithNoValue);

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(
				provider_info.status == ProviderStatus::Registered,
				Error::<T>::NotOperatedProvider
			);
			ensure!(provider_info.owner == who, Error::<T>::NotOwnedProvider);

			let current_era = <CurrentEra<T>>::get().current;
			let mut provider_era_info =
				<ProviderEraInfo<T>>::get(&provider_id, current_era).unwrap_or_default();

			provider_era_info.bond =
				provider_era_info.bond.checked_add(&amount).ok_or(ArithmeticError::Overflow)?;
			provider_era_info.total =
				provider_era_info.total.checked_add(&amount).ok_or(ArithmeticError::Overflow)?;

			T::Currency::reserve(&who, amount)?;

			EraState::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(amount);
				}
			});

			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::ProviderBondedMore { provider_id, amount });

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn provider_bond_less(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(amount > Zero::zero(), Error::<T>::UnstakingWithNoValue);

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(
				provider_info.status == ProviderStatus::Registered,
				Error::<T>::NotOperatedProvider
			);
			ensure!(provider_info.owner == who, Error::<T>::NotOwnedProvider);

			let current_era = <CurrentEra<T>>::get().current;
			let mut provider_era_info =
				<ProviderEraInfo<T>>::get(&provider_id, current_era).unwrap_or_default();

			ensure!(
				provider_era_info.bond >= amount + T::MinProviderStake::get(),
				Error::<T>::InsufficientBond
			);
			provider_era_info.bond = provider_era_info.bond.saturating_sub(amount);
			provider_era_info.total = provider_era_info.total.saturating_sub(amount);

			let mut unbonding_info = <UnbondingInfo<T>>::get(&who);
			unbonding_info.add(UnlockingChunk {
				amount,
				unlock_era: current_era + T::UnbondingPeriod::get(),
			});
			// This should be done after insertion since it's possible for chunks to merge
			ensure!(
				unbonding_info.len() <= T::MaxUnlockingChunks::get(),
				Error::<T>::TooManyUnlockingChunks
			);
			Self::update_unbonding_info(&who, unbonding_info);

			EraState::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(amount);
				}
			});
			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::ProviderBondedLess { provider_id, amount });

			Ok(().into())
		}

		/// Delegate provider, effects of delegation will be felt at the beginning of the next era.
		#[pallet::weight(100)]
		pub fn delegate(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;
			ensure!(amount > Zero::zero(), Error::<T>::StakingWithNoValue);
			ensure!(Self::is_active_provider(&provider_id), Error::<T>::NotOperatedProvider);

			let current_era = <CurrentEra<T>>::get().current;
			let mut provider_era_info =
				<ProviderEraInfo<T>>::get(&provider_id, current_era).unwrap_or_default();
			let mut delegation = <DelegationInfo<T>>::get(&delegator, &provider_id);
			ensure!(
				!delegation.latest_staked_value().is_zero()
					|| provider_era_info.number_of_delegators <= T::MaxDelegatorsPerProvider::get(),
				Error::<T>::MaxNumberOfStakersExceeded
			);
			if delegation.latest_staked_value().is_zero() {
				provider_era_info.number_of_delegators =
					provider_era_info.number_of_delegators.saturating_add(1);
			}

			delegation
				.stake(current_era, amount)
				.map_err(|_| Error::<T>::UnexpectedDelegationInfoEra)?;
			ensure!(
				// One spot should remain for compounding reward claim call
				delegation.len() < T::MaxEraStakeValues::get(),
				Error::<T>::TooManyEraDelegationValues
			);
			ensure!(
				delegation.latest_staked_value() >= T::MinDelegatorStake::get(),
				Error::<T>::InsufficientBond,
			);

			provider_era_info.total =
				provider_era_info.total.checked_add(&amount).ok_or(ArithmeticError::Overflow)?;

			T::Currency::reserve(&delegator, amount)?;

			EraState::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(amount);
				}
			});
			Self::update_delegation_info(&delegator, &provider_id, delegation);
			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::Delegation { delegator, provider_id, amount });

			Ok(().into())
		}

		/// Delegator unstake some funds from the provider. In case remaining bonded balance on
		/// provider is below minimum delegating amount, entire amount for that provider will be
		/// unbonded.
		#[pallet::weight(100)]
		pub fn delegator_unstake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;
			ensure!(amount > Zero::zero(), Error::<T>::UnstakingWithNoValue);
			ensure!(Self::is_active_provider(&provider_id), Error::<T>::NotOperatedProvider);

			let mut delegator_info = <DelegationInfo<T>>::get(&delegator, &provider_id);
			let staked_amount = delegator_info.latest_staked_value();
			ensure!(staked_amount > Zero::zero(), Error::<T>::NotStakedProvider);

			let current_era = <CurrentEra<T>>::get().current;
			let mut provider_era_info =
				<ProviderEraInfo<T>>::get(&provider_id, current_era).unwrap_or_default();

			let remaining = staked_amount.saturating_sub(amount);
			let unstake_amount = if remaining < T::MinDelegatorStake::get() {
				provider_era_info.number_of_delegators =
					provider_era_info.number_of_delegators.saturating_sub(1);
				staked_amount
			} else {
				amount
			};
			provider_era_info.total = provider_era_info.total.saturating_sub(unstake_amount);

			// Sanity check
			ensure!(unstake_amount > Zero::zero(), Error::<T>::UnstakingWithNoValue);

			delegator_info
				.unstake(current_era, unstake_amount)
				.map_err(|_| Error::<T>::UnexpectedDelegationInfoEra)?;
			ensure!(
				// One spot should remain for compounding reward claim call
				delegator_info.len() < T::MaxEraStakeValues::get(),
				Error::<T>::TooManyEraDelegationValues
			);

			let mut unbonding_info = <UnbondingInfo<T>>::get(&delegator);
			unbonding_info.add(UnlockingChunk {
				amount: unstake_amount,
				unlock_era: current_era + T::UnbondingPeriod::get(),
			});
			// This should be done after insertion since it's possible for chunks to merge
			ensure!(
				unbonding_info.len() <= T::MaxUnlockingChunks::get(),
				Error::<T>::TooManyUnlockingChunks
			);
			Self::update_unbonding_info(&delegator, unbonding_info);

			EraState::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(unstake_amount);
				}
			});

			Self::update_delegation_info(&delegator, &provider_id, delegator_info);

			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::DelegatorUnstaked {
				delegator,
				provider_id,
				amount: unstake_amount,
			});

			Ok(().into())
		}

		/// Withdraw all funds that have completed the unbonding process.
		#[pallet::weight(100)]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let mut unbonding_info = <UnbondingInfo<T>>::get(&who);
			let current_era = <CurrentEra<T>>::get().current;

			let (valid_chunks, future_chunks) = unbonding_info.partition(current_era);
			let amount = valid_chunks.sum();

			ensure!(!amount.is_zero(), Error::<T>::NothingToWithdraw);

			unbonding_info = future_chunks;

			T::Currency::unreserve(&who, amount);

			Self::update_unbonding_info(&who, unbonding_info);

			Self::deposit_event(Event::<T>::Withdrawn { who, amount });

			Ok(().into())
		}

		/// Claim earned provider rewards for the specified era.
		#[pallet::weight(100)]
		pub fn claim_provider_reward(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] era: EraIndex,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;

			if let ProviderStatus::Unregistered(unregistered_era) = provider_info.status {
				ensure!(era < unregistered_era, Error::<T>::NotOperatedProvider);
			}

			let current_era = <CurrentEra<T>>::get().current;
			ensure!(era < current_era, Error::<T>::EraOutOfBounds);

			let mut provider_era_info =
				<ProviderEraInfo<T>>::get(&provider_id, era).unwrap_or_default();
			ensure!(
				!provider_era_info.provider_reward_claimed,
				Error::<T>::AlreadyClaimedInThisEra
			);
			ensure!(provider_era_info.total > Zero::zero(), Error::<T>::NotStakedProvider,);

			let era_state = <EraState<T>>::get(era).ok_or(Error::<T>::UnknownEraReward)?;

			let (provider_reward, _) =
				Self::provider_delegators_split(&provider_era_info, &era_state);

			let reward_imbalance = T::Currency::withdraw(
				&Self::account_id(),
				provider_reward,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::AllowDeath,
			)?;
			T::Currency::resolve_creating(&provider_info.owner, reward_imbalance);

			provider_era_info.provider_reward_claimed = true;
			ProviderEraInfo::<T>::insert(&provider_id, era, provider_era_info);

			Self::deposit_event(Event::<T>::Payout {
				who: provider_info.owner.clone(),
				provider_id: provider_id.clone(),
				era,
				amount: provider_reward,
			});

			Ok(().into())
		}

		/// Claim earned delegator rewards for the oldest era.
		#[pallet::weight(100)]
		pub fn claim_delegator_reward(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;

			let mut delegator_info = <DelegationInfo<T>>::get(&delegator, &provider_id);
			let (era, staked) = delegator_info.claim();
			ensure!(staked > Zero::zero(), Error::<T>::NotStakedProvider);

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;
			if let ProviderStatus::Unregistered(unregistered_era) = provider_info.status {
				ensure!(era < unregistered_era, Error::<T>::NotOperatedProvider);
			}

			let current_era = <CurrentEra<T>>::get().current;
			ensure!(era < current_era, Error::<T>::EraOutOfBounds);

			let staking_info = <ProviderEraInfo<T>>::get(&provider_id, era).unwrap_or_default();
			let reward_and_stake = <EraState<T>>::get(era).ok_or(Error::<T>::UnknownEraReward)?;

			let (_, delegators_reward) =
				Self::provider_delegators_split(&staking_info, &reward_and_stake);
			let delegator_reward =
				Perbill::from_rational(staked, staking_info.total) * delegators_reward;

			let reward_imbalance = T::Currency::withdraw(
				&Self::account_id(),
				delegator_reward,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::AllowDeath,
			)?;
			T::Currency::resolve_creating(&delegator, reward_imbalance);

			Self::update_delegation_info(&delegator, &provider_id, delegator_info);

			Self::deposit_event(Event::<T>::Payout {
				who: delegator,
				provider_id,
				era,
				amount: delegator_reward,
			});

			Ok(().into())
		}

		/// Withdraw unregistered provider locked fund.
		#[pallet::weight(100)]
		pub fn provider_withdraw_unregistered(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			let mut provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;
			ensure!(!provider_info.bond_withdrawn, Error::<T>::NothingToWithdraw);

			let unregistered_era = if let ProviderStatus::Unregistered(e) = provider_info.status {
				e
			} else {
				return Err(Error::<T>::NotUnregisteredProvider.into());
			};

			let current_era = <CurrentEra<T>>::get().current;
			ensure!(
				current_era >= unregistered_era + T::UnbondingPeriod::get(),
				Error::<T>::NothingToWithdraw
			);

			let provider_era_info =
				<ProviderEraInfo<T>>::get(&provider_id, unregistered_era).unwrap_or_default();
			let owner = provider_info.owner.clone();
			let withdraw_amount = provider_era_info.bond;

			T::Currency::unreserve(&owner, withdraw_amount);

			let current_era = <CurrentEra<T>>::get().current;
			EraState::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(withdraw_amount);
				}
			});

			provider_info.bond_withdrawn = true;
			ProviderInfo::<T>::insert(&provider_id, provider_info);

			Self::deposit_event(Event::<T>::Withdrawn { who: owner, amount: withdraw_amount });

			Ok(().into())
		}

		/// Withdraw delegator's locked fund from a provider that was unregistered.
		#[pallet::weight(100)]
		pub fn delegator_withdraw_unregistered(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;

			let unregistered_era = if let ProviderStatus::Unregistered(e) = provider_info.status {
				e
			} else {
				return Err(Error::<T>::NotUnregisteredProvider.into());
			};

			let current_era = <CurrentEra<T>>::get().current;
			ensure!(
				current_era >= unregistered_era + T::UnbondingPeriod::get(),
				Error::<T>::NothingToWithdraw
			);

			let mut delegation = <DelegationInfo<T>>::get(&delegator, &provider_id);
			let staked_value = delegation.latest_staked_value();
			ensure!(staked_value > Zero::zero(), Error::<T>::NotStakedProvider);

			// Don't allow withdrawal until all rewards have been claimed.
			let (claimable_era, _) = delegation.claim();
			ensure!(
				claimable_era >= unregistered_era || claimable_era.is_zero(),
				Error::<T>::UnclaimedRewardsRemaining
			);

			T::Currency::unreserve(&delegator, staked_value);

			Self::update_delegation_info(&delegator, &provider_id, Default::default());

			let current_era = <CurrentEra<T>>::get().current;
			EraState::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(staked_value);
				}
			});

			Self::deposit_event(Event::<T>::Withdrawn { who: delegator, amount: staked_value });

			Ok(().into())
		}
	}

	impl<T: Config>
		DapiStaking<
			T::AccountId,
			T::ProviderId,
			<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance,
		> for Pallet<T>
	{
		fn register_provider(
			account: T::AccountId,
			provider_id: T::ProviderId,
			bond: <<T as Config>::Currency as Currency<
				<T as frame_system::Config>::AccountId,
			>>::Balance,
		) -> DispatchResultWithPostInfo {
			ensure!(!ProviderInfo::<T>::contains_key(&provider_id), Error::<T>::ProviderExists);
			ensure!(bond >= T::MinProviderStake::get(), Error::<T>::InsufficientBond);

			T::Currency::reserve(&account, bond)?;

			ProviderInfo::<T>::insert(&provider_id, ProviderMetadata::new(account.clone()));

			let era = <CurrentEra<T>>::get().current;
			ProviderEraInfo::<T>::insert(
				&provider_id,
				era,
				ProviderEraMetadata {
					bond,
					total: bond,
					number_of_delegators: 0,
					provider_reward_claimed: false,
				},
			);

			let mut era_state = <EraState<T>>::get(era).unwrap_or_default();
			era_state.staked = era_state.staked.saturating_add(bond);
			<EraState<T>>::insert(era, era_state);

			Ok(().into())
		}

		fn unregister_provider(provider_id: T::ProviderId) -> DispatchResultWithPostInfo {
			let mut provider =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(provider.status == ProviderStatus::Registered, Error::<T>::NotOperatedProvider);

			let era = <CurrentEra<T>>::get().current;
			provider.status = ProviderStatus::Unregistered(era);
			ProviderInfo::<T>::insert(&provider_id, provider);

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get AccountId assigned to the pallet.
		fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		/// Update the unbonding information for an account.
		fn update_unbonding_info(account: &T::AccountId, info: UnbondingMetadata<BalanceOf<T>>) {
			if info.is_empty() {
				UnbondingInfo::<T>::remove(&account);
			} else {
				UnbondingInfo::<T>::insert(account, info);
			}
		}

		/// Update the delegator info for the `(delegator, provider_id)` pairing.
		/// If delegator info is empty, remove it from the DB. Otherwise, store it.
		fn update_delegation_info(
			delegator: &T::AccountId,
			provider_id: &T::ProviderId,
			info: Delegation<BalanceOf<T>>,
		) {
			if info.is_empty() {
				DelegationInfo::<T>::remove(delegator, provider_id)
			} else {
				DelegationInfo::<T>::insert(delegator, provider_id, info)
			}
		}

		fn era_rewards_snapshot(era: EraIndex) {
			let mut state = <EraState<T>>::get(era).unwrap_or_default();
			EraState::<T>::insert(
				era + 1,
				EraMetadata { rewards: Default::default(), staked: state.staked.clone() },
			);
			state.rewards = RewardAccumulator::<T>::take();
			EraState::<T>::insert(era, state);
		}

		fn rotate_provider_era_info(era: EraIndex) -> u64 {
			let next_era = era + 1;
			let mut consumed_weight = 0;
			for (provider_id, provider_info) in ProviderInfo::<T>::iter() {
				consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
				// Ignore provider if it was unregistered
				if let ProviderStatus::Unregistered(_) = provider_info.status {
					continue;
				}

				// Copy data from era `X` to era `X + 1`
				if let Some(mut provider_era_info) = <ProviderEraInfo<T>>::get(&provider_id, era) {
					provider_era_info.provider_reward_claimed = false;
					ProviderEraInfo::<T>::insert(&provider_id, next_era, provider_era_info);
					consumed_weight =
						consumed_weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
				} else {
					consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
				}
			}

			consumed_weight
		}

		/// `true` if provider is active, `false` if it has been unregistered
		fn is_active_provider(provider_id: &T::ProviderId) -> bool {
			ProviderInfo::<T>::get(provider_id)
				.map_or(false, |provider_info| provider_info.status == ProviderStatus::Registered)
		}

		/// Calculate reward split between provider and delegators.
		///
		/// Returns (provider reward, delegators reward)
		pub(crate) fn provider_delegators_split(
			provider_info: &ProviderEraMetadata<BalanceOf<T>>,
			era_info: &EraMetadata<BalanceOf<T>>,
		) -> (BalanceOf<T>, BalanceOf<T>) {
			let provider_rewards =
				Perbill::from_rational(provider_info.total, era_info.staked) * era_info.rewards;

			let provider_reward_part = T::ProviderRewardsPercentage::get() * provider_rewards;
			let delegators_reward_part = provider_rewards.saturating_sub(provider_reward_part);

			(provider_reward_part, delegators_reward_part)
		}

		/// Handle pallet's imbalance.
		pub fn handle_imbalance(imbalance: NegativeImbalanceOf<T>) {
			RewardAccumulator::<T>::mutate(|v| {
				*v = v.saturating_add(imbalance.peek());
			});
			T::Currency::resolve_creating(&Self::account_id(), imbalance);
		}
	}
}
