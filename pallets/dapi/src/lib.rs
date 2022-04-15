#![cfg_attr(not(feature = "std"), no_std)]

pub mod types;
pub mod weights;

use frame_support::{
	pallet_prelude::DispatchResultWithPostInfo,
	traits::{Currency, ExistenceRequirement, OnUnbalanced, WithdrawReasons},
};
use sp_runtime::traits::Scale;
use sp_std::{collections::btree_set::BTreeSet, prelude::*};

#[cfg(any(feature = "runtime-benchmarks"))]
pub mod benchmarking;
#[cfg(test)]
mod mock;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

type BalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	type AccountIdOf<T> = <T as frame_system::Config>::AccountId;

	/// Blockchain identifier, e.g `eth.mainnet`
	type ChainId<T> = BoundedVec<u8, <T as Config>::ChainIdMaxLength>;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency mechanism
		type Currency: Currency<Self::AccountId>;

		/// dAPI staking.
		type DapiStaking: DapiStaking<Self::AccountId, Self::MassbitId, BalanceOf<Self>>;

		/// The origin which can add/remove regulators.
		type UpdateRegulatorOrigin: EnsureOrigin<Self::Origin>;

		/// For constraining the maximum length of a chain id.
		type ChainIdMaxLength: Get<u32>;

		/// The Id type of Massbit provider or project.
		type MassbitId: Parameter + Member + Default;

		/// Handle project payment as imbalance.
		type OnProjectPayment: OnUnbalanced<
			<Self::Currency as Currency<Self::AccountId>>::NegativeImbalance,
		>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The provider/project is already registered.
		AlreadyExist,
		/// The provider is inactive.
		InactiveProvider,
		/// Chain Id is too long.
		BadChainId,
		/// The provider/project doesn't exist in the list.
		NotExist,
		/// You are not the owner of the The provider/project.
		NotOwner,
		/// No permission to perform specific operation.
		PermissionDenied,
		/// Provider invalid state.
		InvalidProviderState,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New project is registered.
		ProjectRegistered {
			project_id: T::MassbitId,
			consumer: T::AccountId,
			chain_id: Vec<u8>,
			quota: u128,
		},
		/// Project is deposited.
		ProjectDeposited { project_id: T::MassbitId, quota: u128 },
		/// Project reached max quota.
		ProjectReachedQuota { project_id: T::MassbitId },
		/// A provider is registered.
		ProviderRegistered {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			operator: T::AccountId,
			chain_id: Vec<u8>,
		},
		/// Provider is deposited and becomes activated.
		ProviderActivated { provider_id: T::MassbitId, provider_type: ProviderType },
		/// A provider is deactivated by deregistration or reported offence by regulator.
		ProviderDeactivated {
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			reason: ProviderDeactivateReason,
		},
		/// Chain Id is added to well known set.
		ChainIdAdded { chain_id: Vec<u8> },
		/// Chain Id is removed from well known set.
		ChainIdRemoved { chain_id: Vec<u8> },
		/// New regulator is added.
		RegulatorAdded { account_id: T::AccountId },
		/// New regulator is removed.
		RegulatorRemoved { account_id: T::AccountId },
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

			ensure!(!<Projects<T>>::contains_key(&project_id), Error::<T>::AlreadyExist);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;
			ensure!(Self::chain_ids().contains(&bounded_chain_id), Error::<T>::NotExist);

			let payment = T::Currency::withdraw(
				&consumer,
				deposit,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::KeepAlive,
			)?;
			T::OnProjectPayment::on_unbalanced(payment);

			let quota = Self::calculate_quota(deposit);
			let project =
				Project { consumer: consumer.clone(), chain_id: bounded_chain_id, quota, usage: 0 };

			<Projects<T>>::insert(&project_id, project);

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

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::NotExist)?;

			let payment = T::Currency::withdraw(
				&consumer,
				deposit,
				WithdrawReasons::TRANSFER,
				ExistenceRequirement::KeepAlive,
			)?;
			T::OnProjectPayment::on_unbalanced(payment);

			let quota = project.quota.saturating_add(Self::calculate_quota(deposit));
			project.quota = quota;

			<Projects<T>>::insert(&project_id, project);

			Self::deposit_event(Event::ProjectDeposited { project_id, quota });
			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn submit_project_usage(
			origin: OriginFor<T>,
			project_id: T::MassbitId,
			usage: u128,
		) -> DispatchResultWithPostInfo {
			let regulator = ensure_signed(origin)?;
			ensure!(Self::regulators().contains(&regulator), Error::<T>::PermissionDenied);

			let mut project = Projects::<T>::get(&project_id).ok_or(Error::<T>::NotExist)?;
			project.usage = project.usage.saturating_add(usage).min(project.quota);
			if project.usage == project.quota {
				Self::deposit_event(Event::ProjectReachedQuota { project_id: project_id.clone() });
			};

			Projects::<T>::insert(&project_id, project);

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn register_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			provider_type: ProviderType,
			operator: T::AccountId,
			chain_id: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let regulator = ensure_signed(origin)?;
			ensure!(Self::regulators().contains(&regulator), Error::<T>::PermissionDenied);

			ensure!(!<Providers<T>>::contains_key(&provider_id), Error::<T>::AlreadyExist);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;
			ensure!(Self::chain_ids().contains(&bounded_chain_id), Error::<T>::NotExist);

			<Providers<T>>::insert(
				&provider_id,
				Provider {
					provider_type,
					operator: operator.clone(),
					chain_id: bounded_chain_id,
					state: ProviderState::Registered,
				},
			);

			Self::deposit_event(Event::ProviderRegistered {
				provider_id,
				provider_type,
				operator,
				chain_id,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn deposit_provider(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			#[pallet::compact] deposit: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;

			let mut provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::NotExist)?;
			ensure!(provider.operator == operator, Error::<T>::NotOwner);
			ensure!(provider.state == ProviderState::Registered, Error::<T>::InvalidProviderState);

			T::DapiStaking::register(operator.clone(), provider_id.clone(), deposit)?;

			provider.state = ProviderState::Active;
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

			let mut provider = Providers::<T>::get(&provider_id).ok_or(Error::<T>::NotExist)?;
			ensure!(provider.operator == account, Error::<T>::NotOwner);
			ensure!(provider.state == ProviderState::Active, Error::<T>::InactiveProvider);

			T::DapiStaking::unregister(provider_id.clone())?;

			provider.state = ProviderState::InActive;
			Providers::<T>::insert(&provider_id, provider.clone());

			Self::deposit_event(Event::<T>::ProviderDeactivated {
				provider_id,
				provider_type: provider.provider_type,
				reason: ProviderDeactivateReason::UnRegistered,
			});

			Ok(().into())
		}

		#[pallet::weight(100)]
		pub fn report_provider_offence(
			origin: OriginFor<T>,
			provider_id: T::MassbitId,
			reason: ProviderDeactivateReason,
		) -> DispatchResultWithPostInfo {
			let regulator = ensure_signed(origin)?;
			ensure!(Self::regulators().contains(&regulator), Error::<T>::PermissionDenied);

			let mut provider = Self::providers(&provider_id).ok_or(Error::<T>::NotExist)?;
			ensure!(provider.state == ProviderState::Active, Error::<T>::InvalidProviderState);

			T::DapiStaking::unregister(provider_id.clone())?;

			provider.state = ProviderState::InActive;
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
			let _ = ensure_root(origin);

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
			let _ = ensure_root(origin);

			let mut regulators = Regulators::<T>::get();
			ensure!(regulators.contains(&account_id), Error::<T>::NotExist);

			regulators.remove(&account_id);
			Regulators::<T>::put(&regulators);

			Self::deposit_event(Event::RegulatorRemoved { account_id });

			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::add_chain_id())]
		pub fn add_chain_id(origin: OriginFor<T>, chain_id: Vec<u8>) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;

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
			let _ = ensure_root(origin);

			let bounded_chain_id: BoundedVec<u8, T::ChainIdMaxLength> =
				chain_id.clone().try_into().map_err(|_| Error::<T>::BadChainId)?;

			let mut chain_ids = ChainIds::<T>::get();
			ensure!(chain_ids.contains(&bounded_chain_id), Error::<T>::NotExist);

			chain_ids.remove(&bounded_chain_id);
			ChainIds::<T>::put(&chain_ids);

			Self::deposit_event(Event::ChainIdRemoved { chain_id });

			Ok(().into())
		}
	}

	impl<T: Config> Pallet<T> {
		fn calculate_quota(amount: BalanceOf<T>) -> u128 {
			TryInto::<u128>::try_into(amount)
				.ok()
				.unwrap_or_default()
				.div(1_000_000_000_000_000u128)
		}
	}
}

pub trait DapiStaking<AccountId, Provider, Balance> {
	fn register(
		origin: AccountId,
		provider_id: Provider,
		deposit: Balance,
	) -> DispatchResultWithPostInfo;

	fn unregister(provider_id: Provider) -> DispatchResultWithPostInfo;
}
