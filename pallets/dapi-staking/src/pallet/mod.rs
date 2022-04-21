use super::*;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		Currency, ExistenceRequirement, Get, Imbalance, LockIdentifier, LockableCurrency,
		WithdrawReasons,
	},
	weights::Weight,
	PalletId,
};
use frame_system::{ensure_root, ensure_signed};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, Saturating, Zero},
	ArithmeticError, Perbill,
};
use sp_std::convert::From;

use pallet_dapi::DapiStaking;

const STAKING_ID: LockIdentifier = *b"dapistak";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	/// The balance type of this pallet.
	pub type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Negative imbalance type of this pallet.
	type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
		<T as frame_system::Config>::AccountId,
	>>::NegativeImbalance;

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The balance type of pallet.
		type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;

		/// Provider Id.
		type ProviderId: Parameter + Member + Default;

		/// Number of blocks per era.
		#[pallet::constant]
		type BlockPerEra: Get<BlockNumberFor<Self>>;

		/// Percentage of reward paid to delegators.
		#[pallet::constant]
		type ProviderCommission: Get<Perbill>;

		/// Minimum staking amount for new provider registration.
		#[pallet::constant]
		type MinProviderStake: Get<BalanceOf<Self>>;

		/// Maximum number of unique delegators per provider.
		#[pallet::constant]
		type MaxDelegatorsPerProvider: Get<u32>;

		/// Minimum amount delegator must delegate on provider.
		/// Delegator can delegate less if they already have the minimum staking amount delegated on
		/// that particular provider.
		#[pallet::constant]
		type MinDelegatorStake: Get<BalanceOf<Self>>;

		/// dAPI staking pallet Id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Minimum amount that should be left on account after locking.
		#[pallet::constant]
		type MinRemainingAmount: Get<BalanceOf<Self>>;

		/// Max number of unlocking chunks per account Id <-> provider Id pairing.
		/// If value is zero, unlocking becomes impossible.
		#[pallet::constant]
		type MaxUnlockingChunks: Get<u32>;

		/// Number of eras that need to pass until unbonded value can be withdrawn.
		/// Current era is always counted as full era (regardless how much blocks are remaining).
		#[pallet::constant]
		type UnbondingPeriod: Get<u32>;

		/// Max number of unique `EraDelegation` values that can exist for a `(delegator, provider)`
		/// pairing. When delegators claims rewards, they will either keep the number of
		/// `EraDelegation` values the same or they will reduce them by one. Delegators cannot add
		/// an additional `EraDelegation` value by calling `delegate` or `delegator_bond_less` if
		/// they've reached the max number of values.
		///
		/// This ensures that history doesn't grow indefinitely - if there are too many chunks,
		/// delegators should first claim their former rewards before adding additional
		/// `EraDelegation` values.
		#[pallet::constant]
		type MaxEraDelegationValues: Get<u32>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Ledger of an account.
	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AccountLedger<BalanceOf<T>>, ValueQuery>;

	/// The current era index.
	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T> = StorageValue<_, EraIndex, ValueQuery>;

	/// Stores the block number of when the next era starts
	#[pallet::storage]
	#[pallet::getter(fn next_era_start_block)]
	pub type NextEraStartBlock<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery>;

	/// Accumulator for block rewards during an era. It is reset at every new era
	#[pallet::storage]
	#[pallet::getter(fn block_reward_accumulator)]
	pub type BlockRewardAccumulator<T> = StorageValue<_, RewardInfo<BalanceOf<T>>, ValueQuery>;

	#[pallet::type_value]
	pub fn ForceEraOnEmpty() -> Forcing {
		Forcing::NotForcing
	}

	/// Mode of era forcing.
	#[pallet::storage]
	#[pallet::getter(fn force_era)]
	pub type ForceEra<T> = StorageValue<_, Forcing, ValueQuery, ForceEraOnEmpty>;

	/// Registered provider information
	#[pallet::storage]
	#[pallet::getter(fn provider_info)]
	pub(crate) type ProviderInfo<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ProviderId, ProviderMetadata<T::AccountId>>;

	/// Total staked, locked & rewarded for a particular era
	#[pallet::storage]
	#[pallet::getter(fn era_info)]
	pub type EraInfo<T: Config> = StorageMap<_, Twox64Concat, EraIndex, EraSnapshot<BalanceOf<T>>>;

	/// Stores staked amount and delegators for a provider per era
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

	#[pallet::storage]
	#[pallet::getter(fn delegator_info)]
	pub(crate) type DelegatorInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProviderId,
		DelegatorMetadata<BalanceOf<T>>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Account has staked funds on a provider.
		Staked { who: T::AccountId, provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Account has unstaked some funds. Unbonding process begins.
		Unstaked { who: T::AccountId, provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Account has withdrawn unbonded funds.
		Withdrawn { who: T::AccountId, amount: BalanceOf<T> },
		/// New staking era. Distribute era rewards to providers.
		NewEra { era: EraIndex },
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
		/// Can not stake with value less than minimum staking value
		InsufficientValue,
		/// Can not stake with zero value.
		StakingWithNoValue,
		/// Number of stakers per provider exceeded.
		MaxNumberOfStakersExceeded,
		/// Targets must be operated provider
		NotOperatedProvider,
		/// Provider isn't staked.
		NotStakedProvider,
		/// Report issue on github if this is ever emitted
		UnknownEraReward,
		/// Report issue on github if this is ever emitted
		UnexpectedDelegationInfoEra,
		/// Too many active `EraDelegation` values for (delegator, provider) pairing.
		/// Claim existing rewards to fix this problem.
		TooManyEraDelegationValues,
		/// Unclaimed rewards should be claimed before withdrawing stake.
		UnclaimedRewardsRemaining,
		/// Unstaking a provider with zero value
		UnstakingWithNoValue,
		/// Provider has too many unlocking chunks. Withdraw the existing chunks if possible
		/// or wait for current chunks to complete unlocking process to withdraw them.
		TooManyUnlockingChunks,
		/// There are no previously unbonded funds that can be unstaked and withdrawn.
		NothingToWithdraw,
		/// Era parameter is out of bounds
		EraOutOfBounds,
		/// Provider not owned by the account id.
		NotOwnedProvider,
		/// Provider already claimed in this era and reward is distributed
		AlreadyClaimedInThisEra,
		/// Provider isn't unregistered.
		NotUnregisteredProvider,
		/// The provider is already registered by other account
		AlreadyRegisteredProvider,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: BlockNumberFor<T>) -> Weight {
			let force_new_era = Self::force_era().eq(&Forcing::ForceNew);
			let previous_era = Self::current_era();
			let next_era_start_block = Self::next_era_start_block();

			// Value is compared to 1 since genesis block is ignored
			if now >= next_era_start_block || force_new_era || previous_era.is_zero() {
				let blocks_per_era = T::BlockPerEra::get();
				let next_era = previous_era + 1;
				CurrentEra::<T>::put(next_era);

				NextEraStartBlock::<T>::put(now + blocks_per_era);

				let reward = BlockRewardAccumulator::<T>::take();
				Self::reward_balance_snapshot(previous_era, reward);
				let consumed_weight = Self::rotate_staking_info(previous_era);

				if force_new_era {
					ForceEra::<T>::put(Forcing::NotForcing);
				}

				Self::deposit_event(Event::<T>::NewEra { era: next_era });

				consumed_weight + T::DbWeight::get().reads_writes(5, 3)
			} else {
				T::DbWeight::get().reads(4)
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::stake())]
		pub fn provider_stake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;
			ensure!(
				provider_info.status == ProviderStatus::Registered,
				Error::<T>::NotOperatedProvider
			);
			ensure!(provider_info.owner == who, Error::<T>::NotOwnedProvider);

			let mut ledger = Self::ledger(&who);
			let available_balance = Self::available_balance(&who, &ledger);
			let stake_amount = amount.min(available_balance);
			ensure!(stake_amount > Zero::zero(), Error::<T>::StakingWithNoValue);

			let current_era = Self::current_era();
			let mut provider_era_info =
				Self::provider_era_info(&provider_id, current_era).unwrap_or_default();

			// Increment ledger and total delegator value for provider. Overflow shouldn't be
			// possible but the check is here just for safety.
			ledger.locked =
				ledger.locked.checked_add(&stake_amount).ok_or(ArithmeticError::Overflow)?;
			provider_era_info.bond = provider_era_info
				.bond
				.checked_add(&stake_amount)
				.ok_or(ArithmeticError::Overflow)?;
			provider_era_info.total = provider_era_info
				.total
				.checked_add(&stake_amount)
				.ok_or(ArithmeticError::Overflow)?;

			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(stake_amount);
					x.locked = x.locked.saturating_add(stake_amount);
				}
			});

			Self::update_ledger(&who, ledger);
			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::Staked { who, provider_id, amount: stake_amount });

			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::stake())]
		pub fn provider_unstake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure!(amount > Zero::zero(), Error::<T>::UnstakingWithNoValue);
			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;
			ensure!(
				provider_info.status == ProviderStatus::Registered,
				Error::<T>::NotOperatedProvider
			);
			ensure!(provider_info.owner == who, Error::<T>::NotOwnedProvider);

			let current_era = Self::current_era();
			let mut provider_era_info =
				Self::provider_era_info(&provider_id, current_era).unwrap_or_default();

			ensure!(
				provider_era_info.bond >= amount + T::MinProviderStake::get(),
				Error::<T>::InsufficientValue
			);
			provider_era_info.bond = provider_era_info.bond.saturating_sub(amount);
			provider_era_info.total = provider_era_info.total.saturating_sub(amount);

			// Update the chunks
			let mut ledger = Self::ledger(&who);
			ledger.unbonding_info.add(UnlockingChunk {
				amount,
				unlock_era: current_era + T::UnbondingPeriod::get(),
			});
			// This should be done AFTER insertion since it's possible for chunks to merge
			ensure!(
				ledger.unbonding_info.len() <= T::MaxUnlockingChunks::get(),
				Error::<T>::TooManyUnlockingChunks
			);

			Self::update_ledger(&who, ledger);

			// Update total bonded value in era
			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(amount);
				}
			});

			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::Unstaked { who, provider_id, amount });

			Ok(().into())
		}

		/// Delegate provider, effects of delegation will be felt at the beginning of the next era.
		#[pallet::weight(T::WeightInfo::stake())]
		pub fn delegator_stake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;

			ensure!(Self::is_active_provider(&provider_id), Error::<T>::NotOperatedProvider);

			let mut ledger = Self::ledger(&delegator);
			let available_balance = Self::available_balance(&delegator, &ledger);
			let to_stake_amount = amount.min(available_balance);
			ensure!(to_stake_amount > Zero::zero(), Error::<T>::StakingWithNoValue);

			let current_era = Self::current_era();
			let mut provider_era_info =
				Self::provider_era_info(&provider_id, current_era).unwrap_or_default();
			let mut delegator_info = Self::delegator_info(&delegator, &provider_id);

			ensure!(
				!delegator_info.latest_staked_value().is_zero() ||
					provider_era_info.number_of_delegators <= T::MaxDelegatorsPerProvider::get(),
				Error::<T>::MaxNumberOfStakersExceeded
			);
			if delegator_info.latest_staked_value().is_zero() {
				provider_era_info.number_of_delegators =
					provider_era_info.number_of_delegators.saturating_add(1);
			}

			delegator_info
				.stake(current_era, to_stake_amount)
				.map_err(|_| Error::<T>::UnexpectedDelegationInfoEra)?;
			ensure!(
				// One spot should remain for compounding reward claim call
				delegator_info.len() < T::MaxEraDelegationValues::get(),
				Error::<T>::TooManyEraDelegationValues
			);
			ensure!(
				delegator_info.latest_staked_value() >= T::MinDelegatorStake::get(),
				Error::<T>::InsufficientValue,
			);

			// Increment ledger and total delegator value for provider. Overflow shouldn't be
			// possible but the check is here just for safety.
			ledger.locked =
				ledger.locked.checked_add(&to_stake_amount).ok_or(ArithmeticError::Overflow)?;
			provider_era_info.total = provider_era_info
				.total
				.checked_add(&to_stake_amount)
				.ok_or(ArithmeticError::Overflow)?;

			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(to_stake_amount);
					x.locked = x.locked.saturating_add(to_stake_amount);
				}
			});

			Self::update_ledger(&delegator, ledger);
			Self::update_delegator_info(&delegator, &provider_id, delegator_info);
			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::Staked {
				who: delegator,
				provider_id,
				amount: to_stake_amount,
			});

			Ok(().into())
		}

		/// Delegator unstakes from the provider.
		///
		/// The unstaked amount will no longer be eligible for rewards but still won't be unlocked.
		/// User needs to wait for the unbonding period to finish before being able to withdraw
		/// the funds via `withdraw_unbonded` call.
		///
		/// In case remaining bonded balance on provider is below minimum delegating amount,
		/// entire amount for that provider will be unbonded.
		#[pallet::weight(T::WeightInfo::unstake())]
		pub fn delegator_unstake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] amount: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;

			ensure!(amount > Zero::zero(), Error::<T>::UnstakingWithNoValue);
			ensure!(Self::is_active_provider(&provider_id), Error::<T>::NotOperatedProvider);

			let mut delegator_info = Self::delegator_info(&delegator, &provider_id);
			let staked_amount = delegator_info.latest_staked_value();
			ensure!(staked_amount > Zero::zero(), Error::<T>::NotStakedProvider);

			let current_era = Self::current_era();
			let mut provider_era_info =
				Self::provider_era_info(&provider_id, current_era).unwrap_or_default();

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
				delegator_info.len() < T::MaxEraDelegationValues::get(),
				Error::<T>::TooManyEraDelegationValues
			);

			// Update the chunks
			let mut ledger = Self::ledger(&delegator);
			ledger.unbonding_info.add(UnlockingChunk {
				amount: unstake_amount,
				unlock_era: current_era + T::UnbondingPeriod::get(),
			});
			// This should be done AFTER insertion since it's possible for chunks to merge
			ensure!(
				ledger.unbonding_info.len() <= T::MaxUnlockingChunks::get(),
				Error::<T>::TooManyUnlockingChunks
			);

			Self::update_ledger(&delegator, ledger);

			// Update total bonded value in era
			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(unstake_amount);
				}
			});

			Self::update_delegator_info(&delegator, &provider_id, delegator_info);
			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			Self::deposit_event(Event::<T>::Unstaked {
				who: delegator,
				provider_id,
				amount: unstake_amount,
			});

			Ok(().into())
		}

		/// Withdraw all funds that have completed the unbonding process.
		///
		/// If there are unbonding chunks which will be fully unbonded in future eras,
		/// they will remain and can be withdrawn later.
		#[pallet::weight(T::WeightInfo::withdraw_unstaked())]
		pub fn withdraw_unstaked(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&account);
			let current_era = Self::current_era();

			let (valid_chunks, future_chunks) = ledger.unbonding_info.partition(current_era);
			let amount = valid_chunks.sum();

			ensure!(!amount.is_zero(), Error::<T>::NothingToWithdraw);

			ledger.locked = ledger.locked.saturating_sub(amount);
			ledger.unbonding_info = future_chunks;

			Self::update_ledger(&account, ledger);

			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.locked = x.locked.saturating_sub(amount)
				}
			});

			Self::deposit_event(Event::<T>::Withdrawn { who: account, amount });

			Ok(().into())
		}

		/// Claim earned provider rewards for the specified era.
		#[pallet::weight(T::WeightInfo::claim_operator())]
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

			let current_era = Self::current_era();
			ensure!(era < current_era, Error::<T>::EraOutOfBounds);

			let mut provider_era_info =
				Self::provider_era_info(&provider_id, era).unwrap_or_default();
			ensure!(
				!provider_era_info.provider_reward_claimed,
				Error::<T>::AlreadyClaimedInThisEra
			);
			ensure!(provider_era_info.total > Zero::zero(), Error::<T>::NotStakedProvider,);

			let reward_and_stake = Self::era_info(era).ok_or(Error::<T>::UnknownEraReward)?;

			let (provider_reward, _) =
				Self::provider_delegators_split(&provider_era_info, &reward_and_stake);

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
		#[pallet::weight(T::WeightInfo::claim_staker())]
		pub fn claim_delegator_reward(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;

			let mut delegator_info = Self::delegator_info(&delegator, &provider_id);
			let (era, staked) = delegator_info.claim();
			ensure!(staked > Zero::zero(), Error::<T>::NotStakedProvider);

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;
			if let ProviderStatus::Unregistered(unregistered_era) = provider_info.status {
				ensure!(era < unregistered_era, Error::<T>::NotOperatedProvider);
			}

			let current_era = Self::current_era();
			ensure!(era < current_era, Error::<T>::EraOutOfBounds);

			let staking_info = Self::provider_era_info(&provider_id, era).unwrap_or_default();
			let reward_and_stake = Self::era_info(era).ok_or(Error::<T>::UnknownEraReward)?;

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

			Self::update_delegator_info(&delegator, &provider_id, delegator_info);

			Self::deposit_event(Event::<T>::Payout {
				who: delegator,
				provider_id,
				era,
				amount: delegator_reward,
			});

			Ok(().into())
		}

		/// Withdraw unregistered provider locked fund.
		#[pallet::weight(T::WeightInfo::withdraw_from_unregistered_staker())]
		pub fn provider_withdraw(
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
				return Err(Error::<T>::NotUnregisteredProvider.into())
			};

			let current_era = Self::current_era();
			ensure!(
				current_era >= unregistered_era + T::UnbondingPeriod::get(),
				Error::<T>::NothingToWithdraw
			);

			let provider_era_info =
				Self::provider_era_info(&provider_id, unregistered_era).unwrap_or_default();
			let owner = provider_info.owner.clone();
			let withdraw_amount = provider_era_info.bond;

			let mut ledger = Self::ledger(&owner);
			ledger.locked = ledger.locked.saturating_sub(withdraw_amount);
			Self::update_ledger(&owner, ledger);

			let current_era = Self::current_era();
			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(withdraw_amount);
					x.locked = x.locked.saturating_sub(withdraw_amount);
				}
			});

			provider_info.bond_withdrawn = true;
			ProviderInfo::<T>::insert(&provider_id, provider_info);

			Self::deposit_event(Event::<T>::Withdrawn { who: owner, amount: withdraw_amount });

			Ok(().into())
		}

		/// Withdraw delegator's locked fund from a provider that was unregistered.
		#[pallet::weight(T::WeightInfo::withdraw_from_unregistered_staker())]
		pub fn delegator_withdraw(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let delegator = ensure_signed(origin)?;

			let provider_info =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;

			let unregistered_era = if let ProviderStatus::Unregistered(e) = provider_info.status {
				e
			} else {
				return Err(Error::<T>::NotUnregisteredProvider.into())
			};

			let current_era = Self::current_era();
			ensure!(
				current_era >= unregistered_era + T::UnbondingPeriod::get(),
				Error::<T>::NothingToWithdraw
			);

			let mut delegator_info = Self::delegator_info(&delegator, &provider_id);
			let staked_value = delegator_info.latest_staked_value();
			ensure!(staked_value > Zero::zero(), Error::<T>::NotStakedProvider);

			// Don't allow withdrawal until all rewards have been claimed.
			let (claimable_era, _) = delegator_info.claim();
			ensure!(
				claimable_era >= unregistered_era || claimable_era.is_zero(),
				Error::<T>::UnclaimedRewardsRemaining
			);

			let mut ledger = Self::ledger(&delegator);
			ledger.locked = ledger.locked.saturating_sub(staked_value);
			Self::update_ledger(&delegator, ledger);

			Self::update_delegator_info(&delegator, &provider_id, Default::default());

			let current_era = Self::current_era();
			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(staked_value);
					x.locked = x.locked.saturating_sub(staked_value);
				}
			});

			Self::deposit_event(Event::<T>::Withdrawn { who: delegator, amount: staked_value });

			Ok(().into())
		}

		/// Force there to be a new era at the end of the next block. After this, it will be
		/// reset to normal (non-forced) behaviour.
		///
		/// The dispatch origin must be Root.
		///
		///
		/// # <weight>
		/// - No arguments.
		/// - Weight: O(1)
		/// - Write ForceEra
		/// # </weight>
		#[pallet::weight(T::WeightInfo::force_new_era())]
		pub fn force_new_era(origin: OriginFor<T>) -> DispatchResult {
			ensure_root(origin)?;
			ForceEra::<T>::put(Forcing::ForceNew);
			Ok(())
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
			deposit: <<T as Config>::Currency as Currency<
				<T as frame_system::Config>::AccountId,
			>>::Balance,
		) -> DispatchResultWithPostInfo {
			ensure!(
				!ProviderInfo::<T>::contains_key(&provider_id),
				Error::<T>::AlreadyRegisteredProvider
			);

			let mut ledger = Self::ledger(&account);
			let available_balance = Self::available_balance(&account, &ledger);
			let amount = deposit.min(available_balance);
			ensure!(amount >= T::MinProviderStake::get(), Error::<T>::InsufficientValue);

			ProviderInfo::<T>::insert(&provider_id, ProviderMetadata::new(account.clone()));

			let current_era = Self::current_era();
			let mut provider_era_info =
				Self::provider_era_info(&provider_id, current_era).unwrap_or_default();
			provider_era_info.bond =
				provider_era_info.bond.checked_add(&deposit).ok_or(ArithmeticError::Overflow)?;
			provider_era_info.total =
				provider_era_info.total.checked_add(&deposit).ok_or(ArithmeticError::Overflow)?;
			ProviderEraInfo::<T>::insert(&provider_id, current_era, provider_era_info);

			ledger.locked = ledger.locked.checked_add(&deposit).ok_or(ArithmeticError::Overflow)?;
			Self::update_ledger(&account, ledger);

			EraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(amount);
					x.locked = x.locked.saturating_add(amount);
				}
			});

			Ok(().into())
		}

		fn unregister_provider(provider_id: T::ProviderId) -> DispatchResultWithPostInfo {
			let mut provider =
				ProviderInfo::<T>::get(&provider_id).ok_or(Error::<T>::NotOperatedProvider)?;
			ensure!(provider.status == ProviderStatus::Registered, Error::<T>::NotOperatedProvider);

			let current_era = Self::current_era();
			provider.status = ProviderStatus::Unregistered(current_era);
			ProviderInfo::<T>::insert(&provider_id, provider);

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get AccountId assigned to the pallet.
		fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		/// Update the ledger for an account.
		/// This lock will lock the entire funds except paying for further transactions.
		fn update_ledger(account: &T::AccountId, ledger: AccountLedger<BalanceOf<T>>) {
			if ledger.is_empty() {
				Ledger::<T>::remove(&account);
				T::Currency::remove_lock(STAKING_ID, &account);
			} else {
				T::Currency::set_lock(STAKING_ID, &account, ledger.locked, WithdrawReasons::all());
				Ledger::<T>::insert(account, ledger);
			}
		}

		/// Update the delegator info for the `(delegator, provider_id)` pairing.
		/// If delegator info is empty, remove it from the DB. Otherwise, store it.
		fn update_delegator_info(
			delegator: &T::AccountId,
			provider_id: &T::ProviderId,
			metadata: DelegatorMetadata<BalanceOf<T>>,
		) {
			if metadata.is_empty() {
				DelegatorInfo::<T>::remove(delegator, provider_id)
			} else {
				DelegatorInfo::<T>::insert(delegator, provider_id, metadata)
			}
		}

		/// The block rewards are accumulated on the pallet's account during an era.
		/// This function takes a snapshot of the pallet's balance accrued during current era
		/// and stores it for future distribution
		///
		/// This is called just at the beginning of an era.
		fn reward_balance_snapshot(era: EraIndex, rewards: RewardInfo<BalanceOf<T>>) {
			// Get the reward and stake information for previous era
			let mut era_info = Self::era_info(era).unwrap_or_default();

			// Prepare info for the next era
			EraInfo::<T>::insert(
				era + 1,
				EraSnapshot {
					rewards: Default::default(),
					staked: era_info.staked.clone(),
					locked: era_info.locked.clone(),
				},
			);

			// Set reward for the previous era
			era_info.rewards = rewards;
			EraInfo::<T>::insert(era, era_info);
		}

		/// Used to copy all `ProviderEraInfo` from the ending era over to the next era.
		/// This is the most primitive solution since it scales with number of providers.
		/// It is possible to provide a hybrid solution which allows laziness but also prevents
		/// a situation where we don't have access to the required data.
		fn rotate_staking_info(current_era: EraIndex) -> u64 {
			let next_era = current_era + 1;

			let mut consumed_weight = 0;

			for (provider_id, provider_info) in ProviderInfo::<T>::iter() {
				// Ignore provider if it was unregistered
				consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
				if let ProviderStatus::Unregistered(_) = provider_info.status {
					continue
				}

				// Copy data from era `X` to era `X + 1`
				if let Some(mut staking_info) = Self::provider_era_info(&provider_id, current_era) {
					staking_info.provider_reward_claimed = false;
					ProviderEraInfo::<T>::insert(&provider_id, next_era, staking_info);

					consumed_weight =
						consumed_weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
				} else {
					consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
				}
			}

			consumed_weight
		}

		/// Returns available balance
		fn available_balance(
			account: &T::AccountId,
			ledger: &AccountLedger<BalanceOf<T>>,
		) -> BalanceOf<T> {
			// Ensure that staker has enough balance to stake.
			let free_balance =
				T::Currency::free_balance(&account).saturating_sub(T::MinRemainingAmount::get());

			// Remove already locked funds from the free balance
			free_balance.saturating_sub(ledger.locked)
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
			era_info: &EraSnapshot<BalanceOf<T>>,
		) -> (BalanceOf<T>, BalanceOf<T>) {
			let provider_stake_portion =
				Perbill::from_rational(provider_info.total, era_info.staked);

			let provider_reward_part = provider_stake_portion * era_info.rewards.providers;
			let delegators_reward_part = provider_stake_portion * era_info.rewards.delegators;

			(provider_reward_part, delegators_reward_part)
		}

		/// Handle pallet's imbalance (block reward and project payment).
		pub fn handle_imbalance(imbalance: NegativeImbalanceOf<T>) {
			let delegators_part = T::ProviderCommission::get() * imbalance.peek();
			let providers_part = imbalance.peek().saturating_sub(delegators_part);

			BlockRewardAccumulator::<T>::mutate(|accumulated_reward| {
				accumulated_reward.providers =
					accumulated_reward.providers.saturating_add(providers_part);
				accumulated_reward.delegators =
					accumulated_reward.delegators.saturating_add(delegators_part);
			});

			T::Currency::resolve_creating(&Self::account_id(), imbalance);
		}
	}
}
