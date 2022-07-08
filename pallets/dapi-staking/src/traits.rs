use frame_support::pallet_prelude::DispatchResultWithPostInfo;

pub trait DapiStakingRegistration<AccountId, Provider, Balance> {
	fn register_provider(
		origin: AccountId,
		provider_id: Provider,
		deposit: Balance,
	) -> DispatchResultWithPostInfo;

	fn unregister_provider(provider_id: Provider) -> DispatchResultWithPostInfo;
}
