//! dAPI Pallet

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	pallet_prelude::{DispatchResultWithPostInfo, *},
	traits::{Currency, ExistenceRequirement, IsSubType, OnUnbalanced, WithdrawReasons},
};
use sp_runtime::traits::{DispatchInfoOf, Scale, SignedExtension};
use sp_std::{collections::btree_set::BTreeSet, fmt::Debug, prelude::*};

pub mod traits;
pub mod types;
pub mod weights;

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarks;
#[cfg(test)]
mod mock;

pub use pallet::*;
pub use traits::*;
pub use types::*;
pub use weights::WeightInfo;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::*;

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Blockchain identifier, e.g `eth.mainnet`
	type ChainId<T> = BoundedVec<u8, <T as Config>::MaxChainIdLength>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency mechanism.
		type Currency: Currency<Self::AccountId>;

		/// dAPI staking helper.
		type DapiStaking: DapiStaking<Self::AccountId, Self::MassbitId, BalanceOf<Self>>;

		/// The origin which can add/remove regulators.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// For constraining the maximum length of a Chain Id.
		type MaxChainIdLength: Get<u32>;

		/// The id type of Massbit provider or project.
		type MassbitId: Parameter + Member + Default + MaxEncodedLen;

		/// Handle project payment as imbalance.
		type OnProjectPayment: OnUnbalanced<
			<Self::Currency as Currency<Self::AccountId>>::NegativeImbalance,
		>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		ProjectExists,
		ProjectDNE,
		AlreadyExist,
		InactiveProvider,
		ProviderDNE,
		NotOwner,
		PermissionDenied,
		InvalidProviderStatus,
		InvalidChainId,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		ProjectRegistered {
			project_id: T::MassbitId,
			consumer: T::AccountId,
			chain_id: Vec<u8>,
			quota: u128,
		},
		ProjectDeposited {
			project_id: T::MassbitId,
			new_quota: u128,
		},
		ProjectUsageUpdated {
			project_id: T::MassbitId,
			usage: u128,
		},
		ProviderRegistered {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			owner: T::AccountId,
			chain_id: Vec<u8>,
		},
		ProviderActivated {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
		},
		ProviderDeactivated {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			reason: ProviderDeactivateReason,
		},
		ChainIdAdded {
			chain_id: Vec<u8>,
		},
		ChainIdRemoved {
			chain_id: Vec<u8>,
		},
		RegulatorAdded {
			account_id: T::AccountId,
		},
		RegulatorRemoved {
			account_id: T::AccountId,
		},
	}

	#[pallet::storage]
	#[pallet::getter(fn projects)]
	pub(super) type Projects<T: Config> =
		StorageMap<_, Blake2_128Concat, T::MassbitId, Project<AccountIdOf<T>, ChainId<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn providers)]
	pub(super) type Providers<T: Config> =
		StorageMap<_, Blake2_128Concat, T::MassbitId, Provider<AccountIdOf<T>, ChainId<T>>>;

	#[pallet::storage]
	#[pallet::getter(fn regulators)]
	pub type Regulators<T: Config> = StorageValue<_, BTreeSet<T::AccountId>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn chain_ids)]
	pub type ChainIds<T: Config> = StorageValue<_, BTreeSet<ChainId<T>>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub regulators: Vec<T::AccountId>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self { regulators: Vec::new() }
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let regulators =
				&self.regulators.iter().map(|r| r.clone()).collect::<BTreeSet<T::AccountId>>();
			Regulators::<T>::put(&regulators);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::register_project())]
		pub fn register_project(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			chain_id: Vec<u8>,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let consumer = ensure_signed(origin)?;
			ensure!(!<Projects<T>>::contains_key(&project_id), Error::<T>::ProjectExists);
			let bounded_chain_id: BoundedVec<u8, T::MaxChainIdLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::InvalidChainId)?;
			ensure!(Self::chain_ids().contains(&bounded_chain_id), Error::<T>::InvalidChainId);

			let imbalance = T::Currency::withdraw(
				&consumer,
				deposit,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::KeepAlive,
			)?;
			T::OnProjectPayment::on_unbalanced(imbalance);
			let quota = Self::calculate_quota(deposit);
			<Projects<T>>::insert(
				&project_id,
				Project { consumer: consumer.clone(), chain_id: bounded_chain_id, quota, usage: 0 },
			);
			Self::deposit_event(Event::ProjectRegistered { project_id, consumer, chain_id, quota });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::deposit_project())]
		pub fn deposit_project(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let consumer = ensure_signed(origin)?;
			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectDNE)?;
			let quota = project.quota.saturating_add(Self::calculate_quota(deposit));
			project.quota = quota;
			let imbalance = T::Currency::withdraw(
				&consumer,
				deposit,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::KeepAlive,
			)?;
			T::OnProjectPayment::on_unbalanced(imbalance);
			<Projects<T>>::insert(&project_id, project);
			Self::deposit_event(Event::ProjectDeposited { project_id, new_quota: quota });
			Ok(().into())
		}

		#[pallet::weight((0, DispatchClass::Normal, Pays::No))]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			usage: u128,
		) -> DispatchResultWithPostInfo {
			let regulator = ensure_signed(origin)?;
			ensure!(Self::regulators().contains(&regulator), Error::<T>::PermissionDenied);
			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::ProjectDNE)?;
			project.usage = project.usage.saturating_add(usage).min(project.quota);
			let usage = project.usage;
			Projects::<T>::insert(&project_id, project);
			Self::deposit_event(Event::ProjectUsageUpdated { project_id, usage });
			Ok(().into())
		}

		#[pallet::weight((0, DispatchClass::Normal, Pays::No))]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			owner: T::AccountId,
			chain_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let regulator = ensure_signed(origin)?;
			ensure!(<Regulators<T>>::get().contains(&regulator), Error::<T>::PermissionDenied);
			ensure!(!<Providers<T>>::contains_key(&provider_id), Error::<T>::AlreadyExist);
			let bounded_chain_id: BoundedVec<u8, T::MaxChainIdLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::InvalidChainId)?;
			ensure!(<ChainIds<T>>::get().contains(&bounded_chain_id), Error::<T>::InvalidChainId);
			<Providers<T>>::insert(
				&provider_id,
				Provider {
					provider_type,
					owner: owner.clone(),
					chain_id: bounded_chain_id,
					status: ProviderStatus::Registered,
				},
			);
			Self::deposit_event(Event::ProviderRegistered {
				provider_id,
				provider_type,
				owner,
				chain_id,
			});
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::deposit_provider())]
		pub fn deposit_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let owner = ensure_signed(origin)?;
			let mut provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(provider.owner == owner, Error::<T>::NotOwner);
			ensure!(
				provider.status == ProviderStatus::Registered,
				Error::<T>::InvalidProviderStatus
			);
			T::DapiStaking::register_provider(owner.clone(), provider_id.clone(), deposit)?;
			provider.status = ProviderStatus::Active;
			Providers::<T>::insert(&provider_id, provider.clone());
			Self::deposit_event(Event::ProviderActivated {
				provider_id,
				provider_type: provider.provider_type,
			});
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::unregister_provider())]
		pub fn unregister_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
		) -> DispatchResultWithPostInfo {
			let account = ensure_signed(origin)?;
			let mut provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(provider.owner == account, Error::<T>::NotOwner);
			ensure!(provider.status == ProviderStatus::Active, Error::<T>::InvalidProviderStatus);

			T::DapiStaking::unregister_provider(provider_id.clone())?;
			provider.status =
				ProviderStatus::InActive { reason: ProviderDeactivateReason::UnRegistered };
			Providers::<T>::insert(&provider_id, provider.clone());

			Self::deposit_event(Event::<T>::ProviderDeactivated {
				provider_id,
				provider_type: provider.provider_type,
				reason: ProviderDeactivateReason::UnRegistered,
			});
			Ok(().into())
		}

		#[pallet::weight((0, DispatchClass::Normal, Pays::No))]
		pub fn report_provider_offence(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			reason: ProviderDeactivateReason,
		) -> DispatchResultWithPostInfo {
			let regulator = ensure_signed(origin)?;
			ensure!(Self::regulators().contains(&regulator), Error::<T>::PermissionDenied);
			let mut provider = Self::providers(&provider_id).ok_or(Error::<T>::ProviderDNE)?;
			ensure!(provider.status == ProviderStatus::Active, Error::<T>::InvalidProviderStatus);

			T::DapiStaking::unregister_provider(provider_id.clone())?;
			provider.status = ProviderStatus::InActive { reason };
			Providers::<T>::insert(&provider_id, provider.clone());
			Self::deposit_event(Event::<T>::ProviderDeactivated {
				provider_id,
				provider_type: provider.provider_type,
				reason,
			});
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::add_regulator())]
		pub fn add_regulator(
			origin: OriginFor<T>,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let mut regulators = Regulators::<T>::get();
			ensure!(!regulators.contains(&account_id), Error::<T>::AlreadyExist);
			regulators.insert(account_id.clone());
			Regulators::<T>::put(&regulators);
			Self::deposit_event(Event::RegulatorAdded { account_id });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::remove_regulator())]
		pub fn remove_regulator(
			origin: OriginFor<T>,
			account_id: T::AccountId,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let mut regulators = Regulators::<T>::get();
			ensure!(regulators.contains(&account_id), Error::<T>::PermissionDenied);
			regulators.remove(&account_id);
			Regulators::<T>::put(&regulators);
			Self::deposit_event(Event::RegulatorRemoved { account_id });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::add_chain_id())]
		pub fn add_chain_id(origin: OriginFor<T>, chain_id: Vec<u8>) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let bounded_chain_id: BoundedVec<u8, T::MaxChainIdLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::InvalidChainId)?;
			let mut chain_ids = ChainIds::<T>::get();
			ensure!(!chain_ids.contains(&bounded_chain_id), Error::<T>::AlreadyExist);
			chain_ids.insert(bounded_chain_id);
			ChainIds::<T>::put(&chain_ids);
			Self::deposit_event(Event::ChainIdAdded { chain_id });
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::remove_chain_id())]
		pub fn remove_chain_id(
			origin: OriginFor<T>,
			chain_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;
			let bounded_chain_id: BoundedVec<u8, T::MaxChainIdLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::InvalidChainId)?;
			let mut chain_ids = ChainIds::<T>::get();
			ensure!(chain_ids.contains(&bounded_chain_id), Error::<T>::InvalidChainId);
			chain_ids.remove(&bounded_chain_id);
			ChainIds::<T>::put(&chain_ids);
			Self::deposit_event(Event::ChainIdRemoved { chain_id });
			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn calculate_quota(amount: BalanceOf<T>) -> u128 {
			TryInto::<u128>::try_into(amount)
				.ok()
				.unwrap_or_default()
				.div(1_000_000_000_000_000u128)
		}
	}
}

/// Validate regulators calls prior to execution. Needed to avoid a DoS attack since they are
/// otherwise free to place on chain.
#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct PreValidateRegulatorCalls<T: Config + Send + Sync>(sp_std::marker::PhantomData<T>)
where
	<T as frame_system::Config>::Call: IsSubType<Call<T>>;

impl<T: Config + Send + Sync> Debug for PreValidateRegulatorCalls<T>
where
	<T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
	#[cfg(feature = "std")]
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "PreValidateRegulatorCalls")
	}

	#[cfg(not(feature = "std"))]
	fn fmt(&self, _: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		Ok(())
	}
}

impl<T: Config + Send + Sync> PreValidateRegulatorCalls<T>
where
	<T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
	/// Create new `SignedExtension` to check runtime version.
	pub fn new() -> Self {
		Self(sp_std::marker::PhantomData)
	}
}

impl<T: Config + Send + Sync> SignedExtension for PreValidateRegulatorCalls<T>
where
	<T as frame_system::Config>::Call: IsSubType<Call<T>>,
{
	const IDENTIFIER: &'static str = "PreValidateRegulatorCalls";
	type AccountId = T::AccountId;
	type Call = <T as frame_system::Config>::Call;
	type AdditionalSigned = ();
	type Pre = ();

	fn additional_signed(&self) -> Result<Self::AdditionalSigned, TransactionValidityError> {
		Ok(())
	}

	fn pre_dispatch(
		self,
		who: &Self::AccountId,
		call: &Self::Call,
		info: &DispatchInfoOf<Self::Call>,
		len: usize,
	) -> Result<Self::Pre, TransactionValidityError> {
		Ok(self.validate(who, call, info, len).map(|_| ())?)
	}

	fn validate(
		&self,
		who: &Self::AccountId,
		call: &Self::Call,
		_info: &DispatchInfoOf<Self::Call>,
		_len: usize,
	) -> TransactionValidity {
		if let Some(local_call) = call.is_sub_type() {
			match local_call {
				Call::submit_project_usage { .. }
				| Call::register_provider { .. }
				| Call::report_provider_offence { .. } => {
					ensure!(<Regulators<T>>::get().contains(who), InvalidTransaction::BadSigner);
				},
				_ => {},
			}
		}
		Ok(ValidTransaction::default())
	}
}
