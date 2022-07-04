//! Validator Set pallet.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{
		Currency, ExistenceRequirement::KeepAlive, Get, ReservableCurrency, ValidatorRegistration,
	},
	PalletId,
};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedSub, Convert, Saturating, Zero},
	Perbill,
};
use sp_staking::SessionIndex;
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarks;
#[cfg(test)]
mod mock;

pub mod weights;
pub use weights::WeightInfo;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// A convertor from validators id.
	pub struct IdentityValidator;
	impl<T> sp_runtime::traits::Convert<T, Option<T>> for IdentityValidator {
		fn convert(t: T) -> Option<T> {
			Some(t)
		}
	}

	/// Basic information about a candidate.
	#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
	pub struct CandidateInfo<AccountId, Balance> {
		/// Account identifier.
		pub who: AccountId,
		/// Reserved deposit.
		pub deposit: Balance,
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_session::Config {
		/// Overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency mechanism.
		type Currency: ReservableCurrency<Self::AccountId>;

		/// Origin that can dictate updating parameters of this pallet.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// Account identifier from which the internal Pot is generated.
		type PotId: Get<PalletId>;

		/// Maximum number of candidates that we should have. This does not take into account the
		/// invulnerables.
		type MaxCandidates: Get<u32>;

		/// Minimum number of validators that we should have. This is used for disaster recovery.
		///
		/// This does not take into account the invulnerables.
		type MinCandidates: Get<u32>;

		/// Maximum number of invulnerables.
		type MaxInvulnerables: Get<u32>;

		/// Validator will be kicked if block is not produced in threshold.
		type KickThreshold: Get<Self::BlockNumber>;

		/// Validate a user is registered.
		type ValidatorRegistration: ValidatorRegistration<Self::ValidatorId>;

		/// How many in percentage stakes of kicked validators should be slashed (set 0 to disable).
		type SlashRatio: Get<Perbill>;

		/// The weight information of this pallet.
		type WeightInfo: WeightInfo;
	}

	/// The fixed validators.
	#[pallet::storage]
	#[pallet::getter(fn invulnerables)]
	pub type Invulnerables<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

	/// The community validator candidates.
	#[pallet::storage]
	#[pallet::getter(fn candidates)]
	pub type Candidates<T: Config> =
		StorageValue<_, Vec<CandidateInfo<T::AccountId, BalanceOf<T>>>, ValueQuery>;

	/// Last block authored by validator.
	#[pallet::storage]
	#[pallet::getter(fn last_authored_block)]
	pub type LastAuthoredBlock<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, T::BlockNumber, ValueQuery>;

	/// Desired number of candidates.
	///
	/// This should ideally always be less than [`Config::MaxCandidates`] for weights to be correct.
	#[pallet::storage]
	#[pallet::getter(fn desired_candidates)]
	pub type DesiredCandidates<T> = StorageValue<_, u32, ValueQuery>;

	/// Fixed amount to deposit to become a validator.
	///
	/// When a validator calls `leave_intent` they immediately receive the deposit back.
	#[pallet::storage]
	#[pallet::getter(fn candidacy_bond)]
	pub type CandidacyBond<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Destination account for slashed amount.
	#[pallet::storage]
	#[pallet::getter(fn slash_destination)]
	pub type SlashDestination<T> = StorageValue<_, <T as frame_system::Config>::AccountId>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub invulnerables: Vec<T::AccountId>,
		pub candidacy_bond: BalanceOf<T>,
		pub desired_candidates: u32,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				invulnerables: Default::default(),
				candidacy_bond: Default::default(),
				desired_candidates: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let invulnerables = self.invulnerables.iter().collect::<BTreeSet<_>>();
			assert_eq!(
				invulnerables.len(),
				self.invulnerables.len(),
				"duplicate invulnerables in genesis"
			);

			assert!(
				T::MaxInvulnerables::get() >= (self.invulnerables.len() as u32),
				"genesis invulnerables are more than T::MaxInvulnerables",
			);
			assert!(
				T::MaxCandidates::get() >= self.desired_candidates,
				"genesis desired_candidates are more than T::MaxCandidates",
			);

			<Invulnerables<T>>::put(&self.invulnerables);
			<DesiredCandidates<T>>::put(&self.desired_candidates);
			<CandidacyBond<T>>::put(&self.candidacy_bond);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewInvulnerables(Vec<T::AccountId>),
		NewDesiredCandidates(u32),
		NewCandidacyBond(BalanceOf<T>),
		CandidateAdded(T::AccountId, BalanceOf<T>),
		CandidateRemoved(T::AccountId),
		CandidateSlashed(T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		TooManyCandidates,
		TooFewCandidates,
		Unknown,
		Permission,
		AlreadyCandidate,
		NotCandidate,
		AlreadyInvulnerable,
		NoAssociatedValidatorId,
		ValidatorNotRegistered,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the list of invulnerable (fixed) validators.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_invulnerables(new.len() as u32))]
		pub fn set_invulnerables(
			origin: OriginFor<T>,
			new: Vec<T::AccountId>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			// we trust origin calls, this is just a for more accurate benchmarking
			if (new.len() as u32) > T::MaxInvulnerables::get() {
				log::warn!(
					"invulnerables > T::MaxInvulnerables; you might need to run benchmarks again"
				);
			}

			for account_id in &new {
				let validator_key = T::ValidatorIdOf::convert(account_id.clone())
					.ok_or(Error::<T>::NoAssociatedValidatorId)?;
				ensure!(
					T::ValidatorRegistration::is_registered(&validator_key),
					Error::<T>::ValidatorNotRegistered
				);
			}

			<Invulnerables<T>>::put(&new);
			Self::deposit_event(Event::NewInvulnerables(new));
			Ok(().into())
		}

		/// Set the ideal number of validators (not including the invulnerables).
		/// If lowering this number, then the number of running validators could be higher than this
		/// figure. Aside from that edge case, there should be no other way to have more validators
		/// than the desired number.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_desired_candidates())]
		pub fn set_desired_candidates(
			origin: OriginFor<T>,
			max: u32,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			// we trust origin calls, this is just a for more accurate benchmarking
			if max > T::MaxCandidates::get() {
				log::warn!("max > T::MaxCandidates; you might need to run benchmarks again");
			}
			<DesiredCandidates<T>>::put(&max);
			Self::deposit_event(Event::NewDesiredCandidates(max));
			Ok(().into())
		}

		/// Set the candidacy bond amount.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_candidacy_bond())]
		pub fn set_candidacy_bond(
			origin: OriginFor<T>,
			bond: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			<CandidacyBond<T>>::put(&bond);
			Self::deposit_event(Event::NewCandidacyBond(bond));
			Ok(().into())
		}

		/// Register this account as a validator candidate. The account must (a) already have
		/// registered session keys and (b) be able to reserve the `CandidacyBond`.
		///
		/// This call is not available to `Invulnerable` validators.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::register_as_candidate(T::MaxCandidates::get()))]
		pub fn register_as_candidate(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let length = <Candidates<T>>::decode_len().unwrap_or_default();
			ensure!((length as u32) < Self::desired_candidates(), Error::<T>::TooManyCandidates);
			ensure!(!<Invulnerables<T>>::get().contains(&who), Error::<T>::AlreadyInvulnerable);

			let validator_key = T::ValidatorIdOf::convert(who.clone())
				.ok_or(Error::<T>::NoAssociatedValidatorId)?;
			ensure!(
				T::ValidatorRegistration::is_registered(&validator_key),
				Error::<T>::ValidatorNotRegistered
			);

			let deposit = <CandidacyBond<T>>::get();
			let new_candidate = CandidateInfo { who: who.clone(), deposit };
			let _ = <Candidates<T>>::try_mutate(|candidates| -> Result<usize, DispatchError> {
				if candidates.into_iter().any(|candidate| candidate.who == who) {
					Err(Error::<T>::AlreadyCandidate)?
				} else {
					T::Currency::reserve(&who, deposit)?;
					candidates.push(new_candidate);
					<LastAuthoredBlock<T>>::insert(
						who.clone(),
						frame_system::Pallet::<T>::block_number() + T::KickThreshold::get(),
					);
					Ok(candidates.len())
				}
			})?;

			Self::deposit_event(Event::CandidateAdded(who, deposit));
			Ok(().into())
		}

		/// Deregister `origin` as a validator candidate. Note that the validator can only leave on
		/// session change. The `CandidacyBond` will be unreserved immediately.
		///
		/// This call will fail if the total number of candidates would drop below `MinCandidates`.
		///
		/// This call is not available to `Invulnerable` validators.
		#[pallet::weight(<T as pallet::Config>::WeightInfo::leave_intent(T::MaxCandidates::get()))]
		pub fn leave_intent(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				Self::candidates().len() as u32 > T::MinCandidates::get(),
				Error::<T>::TooFewCandidates
			);
			let _ = Self::try_remove_candidate(&who, false)?;
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get a unique, inaccessible account id from the `PotId`.
		pub fn account_id() -> T::AccountId {
			T::PotId::get().into_account()
		}

		/// Removes a candidate if they exist and sends them back their deposit
		/// If second argument is `true` then a candidate will be slashed
		fn try_remove_candidate(who: &T::AccountId, slash: bool) -> Result<usize, DispatchError> {
			let current_count =
				<Candidates<T>>::try_mutate(|candidates| -> Result<usize, DispatchError> {
					let index = candidates
						.iter()
						.position(|candidate| candidate.who == *who)
						.ok_or(Error::<T>::NotCandidate)?;
					let deposit = candidates[index].deposit;

					if slash {
						let slash_amount = T::SlashRatio::get() * deposit;
						let remain = deposit - slash_amount;
						let (imbalance, _) = T::Currency::slash_reserved(&who, slash_amount);
						T::Currency::unreserve(&who, remain);

						if let Some(dest) = Self::slash_destination() {
							T::Currency::resolve_creating(&dest, imbalance);
						}

						Self::deposit_event(Event::CandidateSlashed(who.clone()));
					} else {
						T::Currency::unreserve(&who, deposit);
					}
					candidates.remove(index);
					<LastAuthoredBlock<T>>::remove(who.clone());
					Ok(candidates.len())
				})?;

			Self::deposit_event(Event::CandidateRemoved(who.clone()));
			Ok(current_count)
		}

		/// Assemble the current set of candidates and invulnerables into the next validator set.
		///
		/// This is done on the fly, as frequent as we are told to do so, as the session manager.
		pub fn assemble_validators(candidates: Vec<T::AccountId>) -> Vec<T::AccountId> {
			let mut validators = Self::invulnerables();
			validators.extend(candidates.into_iter().collect::<Vec<_>>());
			validators
		}

		/// Kicks out and candidates that did not produce a block in the kick threshold.
		pub fn kick_stale_candidates(
			candidates: Vec<CandidateInfo<T::AccountId, BalanceOf<T>>>,
		) -> Vec<T::AccountId> {
			let now = frame_system::Pallet::<T>::block_number();
			let kick_threshold = T::KickThreshold::get();
			let new_candidates = candidates
				.into_iter()
				.filter_map(|c| {
					let last_block = <LastAuthoredBlock<T>>::get(&c.who);
					let since_last = now.saturating_sub(last_block);
					if since_last < kick_threshold
						|| Self::candidates().len() as u32 <= T::MinCandidates::get()
					{
						Some(c.who)
					} else {
						let outcome = Self::try_remove_candidate(&c.who, true);
						if let Err(why) = outcome {
							debug_assert!(false, "failed to remove candidate {:?}", why);
						}
						None
					}
				})
				.collect::<Vec<_>>();
			new_candidates
		}
	}
}

/// Keep track of number of authored blocks per authority.
impl<T: Config + pallet_authorship::Config>
	pallet_authorship::EventHandler<T::AccountId, T::BlockNumber> for Pallet<T>
{
	fn note_author(author: T::AccountId) {
		let pot = Self::account_id();
		let reward = T::Currency::free_balance(&pot)
			.checked_sub(&T::Currency::minimum_balance())
			.unwrap_or_else(Zero::zero);
		let success = T::Currency::transfer(&pot, &author, reward, KeepAlive);
		debug_assert!(success.is_ok());
		<LastAuthoredBlock<T>>::insert(author, frame_system::Pallet::<T>::block_number());

		frame_system::Pallet::<T>::register_extra_weight_unchecked(
			<T as pallet::Config>::WeightInfo::note_author(),
			DispatchClass::Mandatory,
		);
	}

	fn note_uncle(_author: T::AccountId, _age: T::BlockNumber) {
		//ignore this
	}
}

impl<T: Config> pallet_session::SessionManager<T::AccountId> for Pallet<T> {
	fn new_session(index: SessionIndex) -> Option<Vec<T::AccountId>> {
		log::info!(
			"assembling new validators for new session {} at #{:?}",
			index,
			<frame_system::Pallet<T>>::block_number(),
		);

		let candidates = Self::candidates();
		let candidates_len_before = candidates.len();
		let active_candidates = Self::kick_stale_candidates(candidates);
		let active_candidates_len = active_candidates.len();
		let result = Self::assemble_validators(active_candidates);
		let removed = candidates_len_before - active_candidates_len;

		frame_system::Pallet::<T>::register_extra_weight_unchecked(
			<T as pallet::Config>::WeightInfo::new_session(
				candidates_len_before as u32,
				removed as u32,
			),
			DispatchClass::Mandatory,
		);
		Some(result)
	}

	fn end_session(_end_index: SessionIndex) {}

	fn start_session(_start_index: SessionIndex) {}
}
