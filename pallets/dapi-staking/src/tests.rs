use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, OnInitialize},
};
use sp_runtime::traits::{AccountIdConversion, Zero};

use crate::{
	pallet::{Error, Event},
	traits::DapiStakingRegistration,
	types::*,
	*,
};
use mock::{Balances, MockProvider, *};

/// Helper struct used to store information relevant to era/provider/delegator combination.
struct MemorySnapshot {
	era_info: EraMetadata<Balance>,
	provider_info: ProviderMetadata<AccountId>,
	delegator_info: Delegation<Balance>,
	provider_era_info: ProviderEraMetadata<Balance>,
	free_balance: Balance,
}

impl MemorySnapshot {
	/// Prepares a new `MemorySnapshot` struct based on the given arguments.
	pub fn all(era: EraIndex, provider_id: &MockProvider, account: AccountId) -> Self {
		Self {
			era_info: DapiStaking::era_state(era).unwrap(),
			provider_info: ProviderInfo::<TestRuntime>::get(provider_id).unwrap(),
			delegator_info: DelegationInfo::<TestRuntime>::get(&account, provider_id),
			provider_era_info: DapiStaking::provider_era_info(provider_id, era).unwrap_or_default(),
			free_balance: <TestRuntime as Config>::Currency::free_balance(&account),
		}
	}

	/// Prepares a new `MemorySnapshot` struct but only with provider-related info
	pub fn provider(era: EraIndex, provider_id: &MockProvider) -> Self {
		Self {
			era_info: DapiStaking::era_state(era).unwrap(),
			provider_info: ProviderInfo::<TestRuntime>::get(provider_id).unwrap(),
			delegator_info: Default::default(),
			provider_era_info: DapiStaking::provider_era_info(provider_id, era).unwrap_or_default(),
			free_balance: Default::default(),
		}
	}
}

/// Free balance of dAPI staking pallet's account
fn free_balance_of_dapi_staking_account() -> Balance {
	<TestRuntime as Config>::Currency::free_balance(&account_id())
}

/// Pallet account Id
fn account_id() -> AccountId {
	<TestRuntime as Config>::PalletId::get().into_account_truncating()
}

/// Total reward for an era.
fn get_total_reward_per_era() -> Balance {
	BLOCK_REWARD * BLOCKS_PER_ERA as Balance
}

fn assert_register(operator: AccountId, provider_id: &MockProvider, stake_amount: Balance) {
	let init_reserved_balance = <TestRuntime as Config>::Currency::reserved_balance(&operator);

	assert!(!ProviderInfo::<TestRuntime>::contains_key(provider_id));

	// Verify op is successfully
	assert_ok!(DapiStaking::register_provider(operator, provider_id.clone(), stake_amount));

	let provider = ProviderInfo::<TestRuntime>::get(provider_id).unwrap();
	assert_eq!(provider.status, ProviderStatus::Active);
	assert_eq!(provider.owner, operator);

	let final_reserved_balance = <TestRuntime as Config>::Currency::reserved_balance(&operator);
	assert_eq!(final_reserved_balance, init_reserved_balance + stake_amount);
}

fn assert_unregister(operator: AccountId, provider_id: &MockProvider) {
	let current_era = DapiStaking::era().current;
	let init_state = MemorySnapshot::provider(current_era, provider_id);

	assert_eq!(init_state.provider_info.status, ProviderStatus::Active);

	assert_ok!(DapiStaking::unregister_provider(provider_id.clone()));

	let final_state = MemorySnapshot::provider(current_era, provider_id);
	assert_eq!(
		final_state.era_info.staked,
		init_state.era_info.staked - init_state.provider_era_info.total
	);
	assert_eq!(final_state.provider_era_info.total, init_state.provider_era_info.total);
	assert_eq!(final_state.provider_info.status, ProviderStatus::Inactive(current_era));
	assert_eq!(final_state.provider_info.owner, operator);
}

fn assert_delegate(delegator: AccountId, provider_id: &MockProvider, value: Balance) {
	let current_era = DapiStaking::era().current;
	let init_state = MemorySnapshot::all(current_era, &provider_id, delegator);
	let staking_value = init_state.free_balance.min(value);

	assert_ok!(DapiStaking::delegate(Origin::signed(delegator), provider_id.clone(), value));
	System::assert_last_event(mock::Event::DapiStaking(Event::Delegated {
		delegator,
		provider_id: provider_id.clone(),
		amount: staking_value,
	}));

	let final_state = MemorySnapshot::all(current_era, &provider_id, delegator);

	// In case delegator hasn't been staking this provider until now
	if init_state.delegator_info.latest_staked_value() == 0 {
		assert!(DelegationInfo::<TestRuntime>::contains_key(&delegator, provider_id));
		assert_eq!(
			final_state.provider_era_info.delegator_count,
			init_state.provider_era_info.delegator_count + 1
		);
	}

	// Verify the remaining states
	assert_eq!(final_state.era_info.staked, init_state.era_info.staked + staking_value);
	assert_eq!(
		final_state.provider_era_info.total,
		init_state.provider_era_info.total + staking_value
	);
	assert_eq!(
		final_state.delegator_info.latest_staked_value(),
		init_state.delegator_info.latest_staked_value() + staking_value
	);
}

#[test]
fn reward_is_ok() {
	ExternalityBuilder::build().execute_with(|| {
		assert_eq!(RewardAccumulator::<TestRuntime>::get(), Default::default());
		assert!(free_balance_of_dapi_staking_account().is_zero());

		let reward = 123456;
		DapiStaking::handle_imbalance(Balances::issue(reward));
		assert_eq!(reward, free_balance_of_dapi_staking_account());

		// After triggering a new era, accumulator should be set to 0 but account shouldn't consume
		// any new imbalance
		DapiStaking::on_initialize(System::block_number());
		assert_eq!(RewardAccumulator::<TestRuntime>::get(), Default::default());
		assert_eq!(reward, free_balance_of_dapi_staking_account());
	});
}

#[test]
fn on_initialize_is_ok() {
	ExternalityBuilder::build().execute_with(|| {
		assert!(DapiStaking::era().current.is_zero());

		// We initialize the first block and advance to second one. New era must be triggered.
		initialize_first_block();
		let current_era = DapiStaking::era().current;
		assert_eq!(1, current_era);

		let previous_era = current_era;
		advance_to_era(previous_era + 10);

		let current_era = DapiStaking::era().current;
		for era in 1..current_era {
			let reward_info = EraState::<TestRuntime>::get(era).unwrap().rewards;
			assert_eq!(get_total_reward_per_era(), reward_info);
		}

		let era_state = EraState::<TestRuntime>::get(current_era).unwrap();
		assert_eq!(0, era_state.staked);
		assert_eq!(0, era_state.rewards);
	})
}

#[test]
fn register_is_ok() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let provider = MockProvider::default();

		assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
		assert_register(operator, &provider, 100);
	})
}

#[test]
fn register_same_provider_twice_fails() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator1 = 1;
		let operator2 = 2;
		let provider = MockProvider::default();

		assert_register(operator1, &provider, 100);

		assert_noop!(
			DapiStaking::register_provider(operator2, provider, 100),
			Error::<TestRuntime>::ProviderExists
		);
	})
}

#[test]
fn unregister_after_register_is_ok() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let provider_id = MockProvider::default();

		assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
		assert_register(operator, &provider_id, 100);
		assert_unregister(operator, &provider_id)
	})
}

#[test]
fn unregister_stake_and_unstake_is_not_ok() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let delegator = 2;
		let provider_id = MockProvider::default();

		assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
		assert_register(operator, &provider_id, 100);
		assert_delegate(delegator, &provider_id, 100);

		assert_unregister(operator, &provider_id);
	})
}
