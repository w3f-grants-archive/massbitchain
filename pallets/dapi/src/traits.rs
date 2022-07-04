use frame_support::dispatch::DispatchResultWithPostInfo;

pub trait DapiStaking<AccountId, Provider, Balance> {
	fn register_provider(
		origin: AccountId,
		provider_id: Provider,
		deposit: Balance,
	) -> DispatchResultWithPostInfo;

	fn unregister_provider(provider_id: Provider) -> DispatchResultWithPostInfo;
}
