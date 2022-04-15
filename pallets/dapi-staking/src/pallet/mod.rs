use super::*;
use frame_support::{
	dispatch::{DispatchResult, RawOrigin},
	ensure,
	traits::{
		Currency, ExistenceRequirement, Get, Imbalance, LockIdentifier, LockableCurrency,
		ReservableCurrency, WithdrawReasons,
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
		/// The staking balance.
		type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>
			+ ReservableCurrency<Self::AccountId>;

		/// Provider Id.
		type ProviderId: Parameter + Member + Default;

		/// Number of blocks per era.
		#[pallet::constant]
		type BlockPerEra: Get<BlockNumberFor<Self>>;

		/// Percentage of reward paid to operator.
		#[pallet::constant]
		type OperatorRewardPercentage: Get<Perbill>;

		/// Minimum bonded deposit for new provider registration.
		#[pallet::constant]
		type RegisterDeposit: Get<BalanceOf<Self>>;

		/// Maximum number of unique stakers per provider.
		#[pallet::constant]
		type MaxNumberOfStakersPerProvider: Get<u32>;

		/// Minimum amount user must stake on provider.
		/// User can stake less if they already have the minimum staking amount staked on that
		/// particular provider.
		#[pallet::constant]
		type MinimumStakingAmount: Get<BalanceOf<Self>>;

		/// dAPI staking pallet Id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Minimum amount that should be left on staker account after staking.
		#[pallet::constant]
		type MinimumRemainingAmount: Get<BalanceOf<Self>>;

		/// Max number of unlocking chunks per account Id <-> provider Id pairing.
		/// If value is zero, unlocking becomes impossible.
		#[pallet::constant]
		type MaxUnlockingChunks: Get<u32>;

		/// Number of eras that need to pass until unstaked value can be withdrawn.
		/// Current era is always counted as full era (regardless how much blocks are remaining).
		#[pallet::constant]
		type UnbondingPeriod: Get<u32>;

		/// Max number of unique `EraStake` values that can exist for a `(staker, provider)`
		/// pairing. When stakers claims rewards, they will either keep the number of `EraStake`
		/// values the same or they will reduce them by one. Stakers cannot add an additional
		/// `EraStake` value by calling `stake` or `unstake` if they've reached the max number of
		/// values.
		///
		/// This ensures that history doesn't grow indefinitely - if there are too many chunks,
		/// stakers should first claim their former rewards before adding additional `EraStake`
		/// values.
		#[pallet::constant]
		type MaxEraStakeValues: Get<u32>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Bonded amount for the staker.
	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, AccountLedger<BalanceOf<T>>, ValueQuery>;

	/// The current era index.
	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T> = StorageValue<_, EraIndex, ValueQuery>;

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
	pub(crate) type RegisteredProviders<T: Config> =
		StorageMap<_, Blake2_128Concat, T::ProviderId, ProviderInfo<T::AccountId>>;

	/// Total staked, locked & rewarded for a particular era
	#[pallet::storage]
	#[pallet::getter(fn general_era_info)]
	pub type GeneralEraInfo<T: Config> =
		StorageMap<_, Twox64Concat, EraIndex, EraInfo<BalanceOf<T>>>;

	/// Stores amount staked and stakers for a provider per era
	#[pallet::storage]
	#[pallet::getter(fn provider_stake_info)]
	pub type ProviderEraStake<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::ProviderId,
		Twox64Concat,
		EraIndex,
		ProviderStakeInfo<BalanceOf<T>>,
	>;

	#[pallet::storage]
	#[pallet::getter(fn staker_info)]
	pub(crate) type GeneralStakerInfo<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		T::ProviderId,
		StakerInfo<BalanceOf<T>>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Account has staked funds on a provider.
		Stake { staker: T::AccountId, provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Account has unbonded & unstaked some funds. Unbonding process begins.
		Unstake { staker: T::AccountId, provider_id: T::ProviderId, amount: BalanceOf<T> },
		/// Account has fully withdrawn all staked amount from an unregistered provider.
		WithdrawFromUnregistered {
			who: T::AccountId,
			provider_id: T::ProviderId,
			amount: BalanceOf<T>,
		},
		/// Account has withdrawn unbonded funds.
		Withdrawn { staker: T::AccountId, amount: BalanceOf<T> },
		/// New dapi staking era. Distribute era rewards to providers.
		NewDapiStakingEra { era: EraIndex },
		/// Reward paid to staker or operator.
		Reward {
			who: T::AccountId,
			provider_id: T::ProviderId,
			era: EraIndex,
			amount: BalanceOf<T>,
		},
		/// Provider removed from dapi staking.
		ProviderUnregistered(T::ProviderId),
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
		UnexpectedStakeInfoEra,
		/// Too many active `EraStake` values for (staker, provider) pairing.
		/// Claim existing rewards to fix this problem.
		TooManyEraStakeValues,
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
			let blocks_per_era = T::BlockPerEra::get();
			let previous_era = Self::current_era();

			// Value is compared to 1 since genesis block is ignored
			if now % blocks_per_era == BlockNumberFor::<T>::from(1u32) ||
				force_new_era || previous_era.is_zero()
			{
				let next_era = previous_era + 1;
				CurrentEra::<T>::put(next_era);

				let reward = BlockRewardAccumulator::<T>::take();
				Self::reward_balance_snapshot(previous_era, reward);
				let consumed_weight = Self::rotate_staking_info(previous_era);

				if force_new_era {
					ForceEra::<T>::put(Forcing::NotForcing);
				}

				Self::deposit_event(Event::<T>::NewDapiStakingEra { era: next_era });

				consumed_weight + T::DbWeight::get().reads_writes(5, 3)
			} else {
				T::DbWeight::get().reads(4)
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Withdraw staker's locked fund from a provider that was unregistered.
		#[pallet::weight(T::WeightInfo::withdraw_from_unregistered_staker())]
		pub fn withdraw_from_unregistered_staker(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let staker = ensure_signed(origin)?;

			let provider_info = RegisteredProviders::<T>::get(&provider_id)
				.ok_or(Error::<T>::NotOperatedProvider)?;

			let unregistered_era = if let ProviderState::Unregistered(e) = provider_info.state {
				e
			} else {
				return Err(Error::<T>::NotUnregisteredProvider.into())
			};

			let current_era = Self::current_era();
			ensure!(
				current_era > unregistered_era + T::UnbondingPeriod::get(),
				Error::<T>::NothingToWithdraw
			);

			let mut staker_info = Self::staker_info(&staker, &provider_id);
			let staked_value = staker_info.latest_staked_value();
			ensure!(staked_value > Zero::zero(), Error::<T>::NotStakedProvider);

			// Don't allow withdrawal until all rewards have been claimed.
			let (claimable_era, _) = staker_info.claim();
			ensure!(
				claimable_era >= unregistered_era || claimable_era.is_zero(),
				Error::<T>::UnclaimedRewardsRemaining
			);

			let mut ledger = Self::ledger(&staker);
			ledger.locked = ledger.locked.saturating_sub(staked_value);
			Self::update_ledger(&staker, ledger);

			Self::update_staker_info(&staker, &provider_id, Default::default());

			let current_era = Self::current_era();
			GeneralEraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(staked_value);
					x.locked = x.locked.saturating_sub(staked_value);
				}
			});

			Self::deposit_event(Event::<T>::WithdrawFromUnregistered {
				who: staker,
				provider_id,
				amount: staked_value,
			});

			Ok(().into())
		}

		/// Withdraw operator's locked fund from a provider that was unregistered.
		#[pallet::weight(T::WeightInfo::withdraw_from_unregistered_operator())]
		pub fn withdraw_from_unregistered_operator(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;

			let mut provider_info = RegisteredProviders::<T>::get(&provider_id)
				.ok_or(Error::<T>::NotOperatedProvider)?;
			ensure!(provider_info.operator == operator, Error::<T>::NotOwnedProvider);
			ensure!(!provider_info.unreserved, Error::<T>::NothingToWithdraw);

			let unregistered_era = if let ProviderState::Unregistered(e) = provider_info.state {
				e
			} else {
				return Err(Error::<T>::NotUnregisteredProvider.into())
			};

			let current_era = Self::current_era();
			ensure!(
				current_era > unregistered_era + T::UnbondingPeriod::get(),
				Error::<T>::NothingToWithdraw
			);

			provider_info.unreserved = true;
			RegisteredProviders::<T>::insert(&provider_id, provider_info);

			let unreserve_amount = T::RegisterDeposit::get();
			T::Currency::unreserve(&operator, unreserve_amount);

			Self::deposit_event(Event::<T>::WithdrawFromUnregistered {
				who: operator,
				provider_id,
				amount: unreserve_amount,
			});

			Ok(().into())
		}

		/// Lock up and stake balance of the origin account.
		///
		/// Effects of staking will be felt at the beginning of the next era.
		#[pallet::weight(T::WeightInfo::stake())]
		pub fn stake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] value: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let staker = ensure_signed(origin)?;

			ensure!(Self::is_active_provider(&provider_id), Error::<T>::NotOperatedProvider);

			let mut ledger = Self::ledger(&staker);
			let available_balance = Self::available_staking_balance(&staker, &ledger);
			let value_to_stake = value.min(available_balance);
			ensure!(value_to_stake > Zero::zero(), Error::<T>::StakingWithNoValue);

			let current_era = Self::current_era();
			let mut staking_info =
				Self::provider_stake_info(&provider_id, current_era).unwrap_or_default();
			let mut staker_info = Self::staker_info(&staker, &provider_id);

			ensure!(
				!staker_info.latest_staked_value().is_zero() ||
					staking_info.number_of_stakers < T::MaxNumberOfStakersPerProvider::get(),
				Error::<T>::MaxNumberOfStakersExceeded
			);
			if staker_info.latest_staked_value().is_zero() {
				staking_info.number_of_stakers = staking_info.number_of_stakers.saturating_add(1);
			}

			staker_info
				.stake(current_era, value_to_stake)
				.map_err(|_| Error::<T>::UnexpectedStakeInfoEra)?;
			ensure!(
				// One spot should remain for compounding reward claim call
				staker_info.len() < T::MaxEraStakeValues::get(),
				Error::<T>::TooManyEraStakeValues
			);
			ensure!(
				staker_info.latest_staked_value() >= T::MinimumStakingAmount::get(),
				Error::<T>::InsufficientValue,
			);

			// Increment ledger and total staker value for provider. Overflow shouldn't be possible
			// but the check is here just for safety.
			ledger.locked =
				ledger.locked.checked_add(&value_to_stake).ok_or(ArithmeticError::Overflow)?;
			staking_info.total = staking_info
				.total
				.checked_add(&value_to_stake)
				.ok_or(ArithmeticError::Overflow)?;

			GeneralEraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_add(value_to_stake);
					x.locked = x.locked.saturating_add(value_to_stake);
				}
			});

			Self::update_ledger(&staker, ledger);
			Self::update_staker_info(&staker, &provider_id, staker_info);
			ProviderEraStake::<T>::insert(&provider_id, current_era, staking_info);

			Self::deposit_event(Event::<T>::Stake { staker, provider_id, amount: value_to_stake });
			Ok(().into())
		}

		/// Start unbonding process and unstake balance from the provider.
		///
		/// The unstaked amount will no longer be eligible for rewards but still won't be unlocked.
		/// User needs to wait for the unbonding period to finish before being able to withdraw
		/// the funds via `withdraw_staked` call.
		///
		/// In case remaining staked balance on provider is below minimum staking amount,
		/// entire stake for that provider will be unstaked.
		#[pallet::weight(T::WeightInfo::unstake())]
		pub fn unstake(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] value: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let staker = ensure_signed(origin)?;

			ensure!(value > Zero::zero(), Error::<T>::UnstakingWithNoValue);
			ensure!(Self::is_active_provider(&provider_id), Error::<T>::NotOperatedProvider);

			let mut staker_info = Self::staker_info(&staker, &provider_id);
			let staked_value = staker_info.latest_staked_value();
			ensure!(staked_value > Zero::zero(), Error::<T>::NotStakedProvider);

			let current_era = Self::current_era();
			let mut provider_stake_info =
				Self::provider_stake_info(&provider_id, current_era).unwrap_or_default();

			let remaining = staked_value.saturating_sub(value);
			let value_to_unstake = if remaining < T::MinimumStakingAmount::get() {
				provider_stake_info.number_of_stakers =
					provider_stake_info.number_of_stakers.saturating_sub(1);
				staked_value
			} else {
				value
			};
			provider_stake_info.total = provider_stake_info.total.saturating_sub(value_to_unstake);

			// Sanity check
			ensure!(value_to_unstake > Zero::zero(), Error::<T>::UnstakingWithNoValue);

			staker_info
				.unstake(current_era, value_to_unstake)
				.map_err(|_| Error::<T>::UnexpectedStakeInfoEra)?;
			ensure!(
				// One spot should remain for compounding reward claim call
				staker_info.len() < T::MaxEraStakeValues::get(),
				Error::<T>::TooManyEraStakeValues
			);

			// Update the chunks
			let mut ledger = Self::ledger(&staker);
			ledger.unbonding_info.add(UnlockingChunk {
				amount: value_to_unstake,
				unlock_era: current_era + T::UnbondingPeriod::get(),
			});
			// This should be done AFTER insertion since it's possible for chunks to merge
			ensure!(
				ledger.unbonding_info.len() <= T::MaxUnlockingChunks::get(),
				Error::<T>::TooManyUnlockingChunks
			);

			Self::update_ledger(&staker, ledger);

			// Update total staked value in era
			GeneralEraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.staked = x.staked.saturating_sub(value_to_unstake);
				}
			});
			Self::update_staker_info(&staker, &provider_id, staker_info);
			ProviderEraStake::<T>::insert(&provider_id, current_era, provider_stake_info);

			Self::deposit_event(Event::<T>::Unstake {
				staker,
				provider_id,
				amount: value_to_unstake,
			});

			Ok(().into())
		}

		/// Withdraw all funds that have completed the unbonding process.
		///
		/// If there are unbonding chunks which will be fully unbonded in future eras,
		/// they will remain and can be withdrawn later.
		#[pallet::weight(T::WeightInfo::withdraw_unstaked())]
		pub fn withdraw_unstaked(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let staker = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&staker);
			let current_era = Self::current_era();

			let (valid_chunks, future_chunks) = ledger.unbonding_info.partition(current_era);
			let withdraw_amount = valid_chunks.sum();

			ensure!(!withdraw_amount.is_zero(), Error::<T>::NothingToWithdraw);

			ledger.locked = ledger.locked.saturating_sub(withdraw_amount);
			ledger.unbonding_info = future_chunks;

			Self::update_ledger(&staker, ledger);
			GeneralEraInfo::<T>::mutate(&current_era, |value| {
				if let Some(x) = value {
					x.locked = x.locked.saturating_sub(withdraw_amount)
				}
			});

			Self::deposit_event(Event::<T>::Withdrawn { staker, amount: withdraw_amount });

			Ok(().into())
		}

		/// Claim earned staker rewards for the oldest era.
		#[pallet::weight(T::WeightInfo::claim_staker())]
		pub fn claim_staker(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
		) -> DispatchResultWithPostInfo {
			let staker = ensure_signed(origin)?;

			let mut staker_info = Self::staker_info(&staker, &provider_id);
			let (era, staked) = staker_info.claim();
			ensure!(staked > Zero::zero(), Error::<T>::NotStakedProvider);

			let provider_info = RegisteredProviders::<T>::get(&provider_id)
				.ok_or(Error::<T>::NotOperatedProvider)?;
			if let ProviderState::Unregistered(unregistered_era) = provider_info.state {
				ensure!(era < unregistered_era, Error::<T>::NotOperatedProvider);
			}

			let current_era = Self::current_era();
			ensure!(era < current_era, Error::<T>::EraOutOfBounds);

			let staking_info = Self::provider_stake_info(&provider_id, era).unwrap_or_default();
			let reward_and_stake =
				Self::general_era_info(era).ok_or(Error::<T>::UnknownEraReward)?;

			let (_, stakers_joint_reward) =
				Self::operator_stakers_split(&staking_info, &reward_and_stake);
			let staker_reward =
				Perbill::from_rational(staked, staking_info.total) * stakers_joint_reward;

			let reward_imbalance = T::Currency::withdraw(
				&Self::account_id(),
				staker_reward,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::AllowDeath,
			)?;
			T::Currency::resolve_creating(&staker, reward_imbalance);

			Self::update_staker_info(&staker, &provider_id, staker_info);

			Self::deposit_event(Event::<T>::Reward {
				who: staker,
				provider_id,
				era,
				amount: staker_reward,
			});

			Ok(().into())
		}

		/// Claim earned operator rewards for the specified era.
		#[pallet::weight(T::WeightInfo::claim_operator())]
		pub fn claim_operator(
			origin: OriginFor<T>,
			provider_id: T::ProviderId,
			#[pallet::compact] era: EraIndex,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			let provider_info = RegisteredProviders::<T>::get(&provider_id)
				.ok_or(Error::<T>::NotOperatedProvider)?;

			let current_era = Self::current_era();
			if let ProviderState::Unregistered(unregistered_era) = provider_info.state {
				ensure!(era < unregistered_era, Error::<T>::NotOperatedProvider);
			}
			ensure!(era < current_era, Error::<T>::EraOutOfBounds);

			let mut provider_stake_info =
				Self::provider_stake_info(&provider_id, era).unwrap_or_default();
			ensure!(
				!provider_stake_info.provider_reward_claimed,
				Error::<T>::AlreadyClaimedInThisEra
			);
			ensure!(provider_stake_info.total > Zero::zero(), Error::<T>::NotStakedProvider,);

			let reward_and_stake =
				Self::general_era_info(era).ok_or(Error::<T>::UnknownEraReward)?;

			let (operator_reward, _) =
				Self::operator_stakers_split(&provider_stake_info, &reward_and_stake);

			let reward_imbalance = T::Currency::withdraw(
				&Self::account_id(),
				operator_reward,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::AllowDeath,
			)?;
			T::Currency::resolve_creating(&provider_info.operator, reward_imbalance);

			provider_stake_info.provider_reward_claimed = true;
			ProviderEraStake::<T>::insert(&provider_id, era, provider_stake_info);

			Self::deposit_event(Event::<T>::Reward {
				who: provider_info.operator.clone(),
				provider_id: provider_id.clone(),
				era,
				amount: operator_reward,
			});

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
		fn register(
			operator: T::AccountId,
			provider_id: T::ProviderId,
			deposit: <<T as Config>::Currency as Currency<
				<T as frame_system::Config>::AccountId,
			>>::Balance,
		) -> DispatchResultWithPostInfo {
			ensure!(
				!RegisteredProviders::<T>::contains_key(&provider_id),
				Error::<T>::AlreadyRegisteredProvider
			);

			let register_deposit = T::RegisterDeposit::get();
			ensure!(
				deposit >= register_deposit + T::MinimumStakingAmount::get(),
				Error::<T>::InsufficientValue
			);

			T::Currency::reserve(&operator, register_deposit)?;

			RegisteredProviders::<T>::insert(&provider_id, ProviderInfo::new(operator.clone()));

			let stake_amount = deposit.saturating_sub(register_deposit);
			Self::stake(RawOrigin::Signed(operator).into(), provider_id, stake_amount)?;

			Ok(().into())
		}

		fn unregister(provider_id: T::ProviderId) -> DispatchResultWithPostInfo {
			let mut provider_info = RegisteredProviders::<T>::get(&provider_id)
				.ok_or(Error::<T>::NotOperatedProvider)?;
			ensure!(
				provider_info.state == ProviderState::Registered,
				Error::<T>::NotOperatedProvider
			);

			let current_era = Self::current_era();
			provider_info.state = ProviderState::Unregistered(current_era);
			RegisteredProviders::<T>::insert(&provider_id, provider_info);

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get AccountId assigned to the pallet.
		fn account_id() -> T::AccountId {
			T::PalletId::get().into_account()
		}

		/// Update the ledger for a staker. This will also update the stash lock.
		/// This lock will lock the entire funds except paying for further transactions.
		fn update_ledger(staker: &T::AccountId, ledger: AccountLedger<BalanceOf<T>>) {
			if ledger.is_empty() {
				Ledger::<T>::remove(&staker);
				T::Currency::remove_lock(STAKING_ID, &staker);
			} else {
				T::Currency::set_lock(STAKING_ID, &staker, ledger.locked, WithdrawReasons::all());
				Ledger::<T>::insert(staker, ledger);
			}
		}

		/// Update the staker info for the `(staker, provider_id)` pairing.
		/// If staker_info is empty, remove it from the DB. Otherwise, store it.
		fn update_staker_info(
			staker: &T::AccountId,
			provider_id: &T::ProviderId,
			staker_info: StakerInfo<BalanceOf<T>>,
		) {
			if staker_info.is_empty() {
				GeneralStakerInfo::<T>::remove(staker, provider_id)
			} else {
				GeneralStakerInfo::<T>::insert(staker, provider_id, staker_info)
			}
		}

		/// The block rewards are accumulated on the pallet's account during an era.
		/// This function takes a snapshot of the pallet's balance accrued during current era
		/// and stores it for future distribution
		///
		/// This is called just at the beginning of an era.
		fn reward_balance_snapshot(era: EraIndex, rewards: RewardInfo<BalanceOf<T>>) {
			// Get the reward and stake information for previous era
			let mut era_info = Self::general_era_info(era).unwrap_or_default();

			// Prepare info for the next era
			GeneralEraInfo::<T>::insert(
				era + 1,
				EraInfo {
					rewards: Default::default(),
					staked: era_info.staked.clone(),
					locked: era_info.locked.clone(),
				},
			);

			// Set reward for the previous era
			era_info.rewards = rewards;
			GeneralEraInfo::<T>::insert(era, era_info);
		}

		/// Used to copy all `ProviderStakeInfo` from the ending era over to the next era.
		/// This is the most primitive solution since it scales with number of providers.
		/// It is possible to provide a hybrid solution which allows laziness but also prevents
		/// a situation where we don't have access to the required data.
		fn rotate_staking_info(current_era: EraIndex) -> u64 {
			let next_era = current_era + 1;

			let mut consumed_weight = 0;

			for (provider_id, provider_info) in RegisteredProviders::<T>::iter() {
				// Ignore provider if it was unregistered
				consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
				if let ProviderState::Unregistered(_) = provider_info.state {
					continue
				}

				// Copy data from era `X` to era `X + 1`
				if let Some(mut staking_info) = Self::provider_stake_info(&provider_id, current_era)
				{
					staking_info.provider_reward_claimed = false;
					ProviderEraStake::<T>::insert(&provider_id, next_era, staking_info);

					consumed_weight =
						consumed_weight.saturating_add(T::DbWeight::get().reads_writes(1, 1));
				} else {
					consumed_weight = consumed_weight.saturating_add(T::DbWeight::get().reads(1));
				}
			}

			consumed_weight
		}

		/// Returns available staking balance for the potential staker
		fn available_staking_balance(
			staker: &T::AccountId,
			ledger: &AccountLedger<BalanceOf<T>>,
		) -> BalanceOf<T> {
			// Ensure that staker has enough balance to stake.
			let free_balance =
				T::Currency::free_balance(&staker).saturating_sub(T::MinimumRemainingAmount::get());

			// Remove already locked funds from the free balance
			free_balance.saturating_sub(ledger.locked)
		}

		/// `true` if provider is active, `false` if it has been unregistered
		fn is_active_provider(provider_id: &T::ProviderId) -> bool {
			RegisteredProviders::<T>::get(provider_id)
				.map_or(false, |provider_info| provider_info.state == ProviderState::Registered)
		}

		/// Calculate reward split between operator and stakers.
		///
		/// Returns (operator reward, joint stakers reward)
		pub(crate) fn operator_stakers_split(
			provider_info: &ProviderStakeInfo<BalanceOf<T>>,
			era_info: &EraInfo<BalanceOf<T>>,
		) -> (BalanceOf<T>, BalanceOf<T>) {
			let provider_stake_portion =
				Perbill::from_rational(provider_info.total, era_info.staked);

			let operator_reward_part = provider_stake_portion * era_info.rewards.operators;
			let stakers_reward_part = provider_stake_portion * era_info.rewards.stakers;

			(operator_reward_part, stakers_reward_part)
		}

		/// Adds rewards to the reward pool.
		pub fn rewards(imbalance: NegativeImbalanceOf<T>) {
			let operators_part = T::OperatorRewardPercentage::get() * imbalance.peek();
			let stakers_part = imbalance.peek().saturating_sub(operators_part);

			BlockRewardAccumulator::<T>::mutate(|accumulated_reward| {
				accumulated_reward.operators =
					accumulated_reward.operators.saturating_add(operators_part);
				accumulated_reward.stakers =
					accumulated_reward.stakers.saturating_add(stakers_part);
			});

			T::Currency::resolve_creating(&Self::account_id(), imbalance);
		}
	}
}
