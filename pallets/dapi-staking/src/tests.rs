use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, OnInitialize},
};
use sp_arithmetic::Perbill;
use sp_runtime::traits::{AccountIdConversion, Zero};

use crate::{
	pallet::{Error, Event},
	traits::DapiStakingRegistration,
	types::*,
	*,
};
use common::MassbitId;
use mock::{Balances, *};

/// Helper struct used to store information relevant to era/provider/delegator combination.
struct MemorySnapshot {
	era_info: EraMetadata<Balance>,
	provider_info: ProviderMetadata<AccountId>,
	delegator_info: Delegation<Balance>,
	provider_era_info: ProviderEraMetadata<Balance>,
	free_balance: Balance,
	unbonding_info: UnbondingMetadata<Balance>,
}

impl MemorySnapshot {
	/// Prepares a new `MemorySnapshot` struct based on the given arguments.
	pub fn all(era: EraIndex, provider_id: &MassbitId, account: AccountId) -> Self {
		Self {
			era_info: DapiStaking::era_state(era).unwrap(),
			provider_info: ProviderInfo::<TestRuntime>::get(provider_id).unwrap(),
			delegator_info: DelegationInfo::<TestRuntime>::get(&account, provider_id),
			provider_era_info: DapiStaking::provider_era_info(provider_id, era).unwrap_or_default(),
			free_balance: <TestRuntime as Config>::Currency::free_balance(&account),
			unbonding_info: UnbondingInfo::<TestRuntime>::get(&account),
		}
	}

	/// Prepares a new `MemorySnapshot` struct but only with provider-related info
	pub fn provider(era: EraIndex, provider_id: &MassbitId) -> Self {
		Self {
			era_info: DapiStaking::era_state(era).unwrap(),
			provider_info: ProviderInfo::<TestRuntime>::get(provider_id).unwrap(),
			delegator_info: Default::default(),
			provider_era_info: DapiStaking::provider_era_info(provider_id, era).unwrap_or_default(),
			free_balance: Default::default(),
			unbonding_info: Default::default(),
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

fn assert_register(operator: AccountId, provider_id: &MassbitId, stake_amount: Balance) {
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

fn assert_unregister(operator: AccountId, provider_id: &MassbitId) {
	let current_era = DapiStaking::current_era();
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

fn assert_delegate(delegator: AccountId, provider_id: &MassbitId, amount: Balance) {
	let current_era = DapiStaking::current_era();
	let init_state = MemorySnapshot::all(current_era, &provider_id, delegator);
	let staking_amount = init_state.free_balance.min(amount);

	assert_ok!(DapiStaking::delegate(Origin::signed(delegator), provider_id.clone(), amount));
	System::assert_last_event(mock::Event::DapiStaking(Event::Delegated {
		delegator,
		provider_id: provider_id.clone(),
		amount: staking_amount,
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
	assert_eq!(final_state.era_info.staked, init_state.era_info.staked + staking_amount);
	assert_eq!(
		final_state.provider_era_info.total,
		init_state.provider_era_info.total + staking_amount
	);
	assert_eq!(
		final_state.delegator_info.latest_staked_value(),
		init_state.delegator_info.latest_staked_value() + staking_amount
	);
}

fn assert_delegator_unstake(delegator: AccountId, provider_id: &MassbitId, amount: Balance) {
	let current_era = DapiStaking::current_era();
	let init_state = MemorySnapshot::all(current_era, &provider_id, delegator);

	assert_ok!(DapiStaking::delegator_unstake(
		Origin::signed(delegator),
		provider_id.clone(),
		amount
	));
	System::assert_last_event(mock::Event::DapiStaking(Event::DelegatorUnstaked {
		delegator,
		provider_id: provider_id.clone(),
		amount,
	}));

	let final_state = MemorySnapshot::all(current_era, &provider_id, delegator);
	let expected_unlock_era = current_era + UNBONDING_PERIOD;
	match init_state
		.unbonding_info
		.vec()
		.binary_search_by(|x| x.unlock_era.cmp(&expected_unlock_era))
	{
		Ok(_) => assert_eq!(init_state.unbonding_info.len(), final_state.unbonding_info.len()),
		Err(_) => assert_eq!(init_state.unbonding_info.len() + 1, final_state.unbonding_info.len()),
	}
}

fn assert_claim_delegator(claimer: AccountId, provider_id: &MassbitId) {
	let (claim_era, _) = <DelegationInfo<TestRuntime>>::get(&claimer, provider_id).claim();

	// clean up possible leftover events
	System::reset_events();

	let init_state_claim_era = MemorySnapshot::all(claim_era, provider_id, claimer);

	let (_, delegators_reward) = DapiStaking::split_provider_delegators_rewards(
		&init_state_claim_era.provider_era_info,
		&init_state_claim_era.era_info,
	);

	let (claim_era, staked) = init_state_claim_era.delegator_info.clone().claim();
	assert!(claim_era > 0);

	// Cannot claim rewards post unregister era, this indicates a bug!
	if let ProviderStatus::Inactive(unregistered_era) = init_state_claim_era.provider_info.status {
		assert!(unregistered_era > claim_era);
	}

	let calculated_reward = Perbill::from_rational(
		staked,
		init_state_claim_era
			.provider_era_info
			.total
			.saturating_sub(init_state_claim_era.provider_era_info.bond),
	) * delegators_reward;
	let issuance_before_claim = <TestRuntime as Config>::Currency::total_issuance();

	assert_ok!(DapiStaking::claim_delegator(Origin::signed(claimer), provider_id.clone()));

	let final_state_current_era = MemorySnapshot::all(claim_era, provider_id, claimer);

	System::assert_last_event(mock::Event::DapiStaking(Event::Payout {
		who: claimer,
		provider_id: provider_id.clone(),
		era: claim_era,
		amount: calculated_reward,
	}));

	let (new_era, _) = final_state_current_era.delegator_info.clone().claim();
	if final_state_current_era.delegator_info.is_empty() {
		assert!(new_era.is_zero());
		assert!(!DelegationInfo::<TestRuntime>::contains_key(&claimer, provider_id));
	} else {
		assert!(new_era > claim_era);
	}
	assert!(new_era.is_zero() || new_era > claim_era);

	// Claim shouldn't mint new tokens, instead it should just transfer from the dapi staking pallet
	// account
	let issuance_after_claim = <TestRuntime as Config>::Currency::total_issuance();
	assert_eq!(issuance_before_claim, issuance_after_claim);

	let final_state_claim_era = MemorySnapshot::all(claim_era, provider_id, claimer);
	assert_eq!(init_state_claim_era.provider_info, final_state_claim_era.provider_info);
}

fn assert_claim_provider(provider_id: &MassbitId, claim_era: EraIndex) {
	let operator = DapiStaking::provider_info(provider_id).unwrap().owner;
	let init_state = MemorySnapshot::all(claim_era, provider_id, operator);

	if let ProviderStatus::Inactive(unregistered_era) = init_state.provider_info.status {
		assert!(unregistered_era > claim_era);
	}

	let (calculated_reward, _) = DapiStaking::split_provider_delegators_rewards(
		&init_state.provider_era_info,
		&init_state.era_info,
	);

	assert_ok!(DapiStaking::claim_provider(
		Origin::signed(operator),
		provider_id.clone(),
		claim_era
	));
	System::assert_last_event(mock::Event::DapiStaking(Event::Payout {
		who: operator,
		provider_id: provider_id.clone(),
		era: claim_era,
		amount: calculated_reward,
	}));

	let final_state = MemorySnapshot::all(claim_era, provider_id, operator);
	assert_eq!(init_state.free_balance + calculated_reward, final_state.free_balance);

	assert!(final_state.provider_era_info.provider_reward_claimed);

	assert_eq!(init_state.delegator_info, final_state.delegator_info);
	assert_eq!(init_state.unbonding_info, final_state.unbonding_info);
}

/// Perform `delegator_withdraw_unregistered` with all the accompanied checks including before/after
/// storage comparison.
fn assert_delegator_withdraw_from_unregistered(delegator: AccountId, provider_id: &MassbitId) {
	let current_era = DapiStaking::current_era();
	let init_state = MemorySnapshot::all(current_era, provider_id, delegator);

	if let ProviderStatus::Inactive(era) = init_state.provider_info.status {
		assert!(era <= current_era);
	} else {
		panic!("Provider should be inactive.")
	};

	let staked_value = init_state.delegator_info.latest_staked_value();
	assert!(staked_value > 0);

	assert_ok!(DapiStaking::delegator_withdraw_unregistered(
		Origin::signed(delegator),
		provider_id.clone()
	));
	System::assert_last_event(mock::Event::DapiStaking(Event::Withdrawn {
		who: delegator,
		amount: staked_value,
	}));

	let final_state = MemorySnapshot::all(current_era, provider_id, delegator);
	assert_eq!(init_state.provider_info, final_state.provider_info);
	assert_eq!(init_state.unbonding_info, final_state.unbonding_info);
	assert!(final_state.delegator_info.latest_staked_value().is_zero());
	assert!(!DelegationInfo::<TestRuntime>::contains_key(&delegator, provider_id));
}

/// Perform `withdraw_from_unregistered` with all the accompanied checks including before/after
/// storage comparison.
fn assert_provider_withdraw_from_unregistered(provider_id: &MassbitId) {
	let current_era = DapiStaking::current_era();
	let operator = DapiStaking::provider_info(provider_id).unwrap().owner;
	let init_state = MemorySnapshot::all(current_era, provider_id, operator);

	let unregistered_era = if let ProviderStatus::Inactive(era) = init_state.provider_info.status {
		assert!(era <= current_era);
		era
	} else {
		panic!("Provider should be inactive.")
	};

	let provider_era_info =
		<ProviderEraInfo<TestRuntime>>::get(&provider_id, unregistered_era).unwrap_or_default();
	let bonded_amount = provider_era_info.bond;
	assert!(bonded_amount > 0);

	assert_ok!(DapiStaking::provider_withdraw_unregistered(
		Origin::signed(operator),
		provider_id.clone()
	));
	System::assert_last_event(mock::Event::DapiStaking(Event::Withdrawn {
		who: operator,
		amount: bonded_amount,
	}));

	let final_state = MemorySnapshot::all(current_era, provider_id, operator);
	assert!(final_state.provider_info.bond_withdrawn);
	assert_eq!(init_state.free_balance + bonded_amount, final_state.free_balance);
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
		let current_era = DapiStaking::current_era();
		assert_eq!(1, current_era);

		let previous_era = current_era;
		advance_to_era(previous_era + 10);

		let current_era = DapiStaking::current_era();
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
fn new_era_length_is_always_blocks_per_era() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();
		let block_per_era = BLOCKS_PER_ERA;

		advance_to_era(mock::DapiStaking::current_era() + 1);

		let start_era = mock::DapiStaking::current_era();
		let start_block_number = System::block_number();

		advance_to_era(mock::DapiStaking::current_era() + 1);
		let end_block_number = System::block_number();

		assert_eq!(mock::DapiStaking::current_era(), start_era + 1);
		assert_eq!(end_block_number - start_block_number, block_per_era as u64);
	})
}

#[test]
fn new_era_is_ok() {
	ExternalityBuilder::build().execute_with(|| {
		advance_to_era(DapiStaking::current_era() + 10);
		let start_era = DapiStaking::current_era();

		assert_eq!(DapiStaking::reward_accumulator(), Default::default());

		run_for_blocks(1);
		let current_era = DapiStaking::current_era();
		assert_eq!(start_era, current_era);

		// verify that block reward is added to the block_reward_accumulator
		let block_reward = DapiStaking::reward_accumulator();
		assert_eq!(BLOCK_REWARD, block_reward);

		let delegator = 2;
		let delegated_amount = 100;
		let operator = 3;
		let bonded_amount = 10;
		let staked_amount = delegated_amount + bonded_amount;
		let provider = MassbitId::repeat_byte(0x01);
		assert_register(operator, &provider, bonded_amount);
		assert_delegate(delegator, &provider, delegated_amount);

		advance_to_era(DapiStaking::current_era() + 1);

		let current_era = DapiStaking::current_era();
		assert_eq!(start_era + 1, current_era);
		let current_block = System::block_number();
		System::assert_last_event(mock::Event::DapiStaking(Event::NewEra {
			era: current_era,
			first_block: current_block,
		}));

		let block_reward = DapiStaking::reward_accumulator();
		assert_eq!(block_reward, Default::default());

		let expected_era_reward = get_total_reward_per_era();
		let era_rewards = EraState::<TestRuntime>::get(start_era).unwrap();
		assert_eq!(staked_amount, era_rewards.staked);
		assert_eq!(expected_era_reward, era_rewards.rewards);
	})
}

#[test]
fn register_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let provider = MassbitId::repeat_byte(0x01);

		assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
		assert_register(operator, &provider, 100);
	})
}

#[test]
fn register_same_provider_twice_fail() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator1 = 1;
		let operator2 = 2;
		let provider = MassbitId::repeat_byte(0x01);

		assert_register(operator1, &provider, 100);

		assert_noop!(
			DapiStaking::register_provider(operator2, provider, 100),
			Error::<TestRuntime>::ProviderExists
		);
	})
}

#[test]
fn unregister_after_register_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let provider_id = MassbitId::repeat_byte(0x01);

		assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
		assert_register(operator, &provider_id, 100);
		assert_unregister(operator, &provider_id)
	})
}

#[test]
fn unregister_stake_and_unstake_fail() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let delegator = 2;
		let provider_id = MassbitId::repeat_byte(0x01);

		assert!(<TestRuntime as Config>::Currency::reserved_balance(&operator).is_zero());
		assert_register(operator, &provider_id, 100);
		assert_delegate(delegator, &provider_id, 100);

		assert_unregister(operator, &provider_id);

		assert_noop!(
			DapiStaking::provider_bond_more(Origin::signed(operator), provider_id.clone(), 100),
			Error::<TestRuntime>::NotOperatedProvider
		);
		assert_noop!(
			DapiStaking::provider_bond_less(Origin::signed(operator), provider_id.clone(), 100),
			Error::<TestRuntime>::NotOperatedProvider
		);
	})
}

#[test]
fn withdraw_from_unregistered_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let bond_amount = 100;
		let delegator = 3;
		let delegate_amount = 100;
		let provider = MassbitId::repeat_byte(0x01);

		assert_register(operator, &provider, bond_amount);
		assert_delegate(delegator, &provider, delegate_amount);

		advance_to_era(5);

		for era in 1..DapiStaking::current_era() {
			assert_claim_delegator(delegator, &provider);
			assert_claim_provider(&provider, era);
		}

		assert_unregister(operator, &provider);
		advance_to_era(9);
		assert_delegator_withdraw_from_unregistered(delegator, &provider);
		assert_provider_withdraw_from_unregistered(&provider);
		// No additional claim ops should be possible
		assert_noop!(
			DapiStaking::claim_delegator(Origin::signed(delegator), provider.clone()),
			Error::<TestRuntime>::NotStakedProvider
		);
		assert_noop!(
			DapiStaking::claim_provider(
				Origin::signed(operator),
				provider.clone(),
				DapiStaking::current_era()
			),
			Error::<TestRuntime>::NotOperatedProvider
		);
	})
}

#[test]
fn withdraw_from_unregistered_fail_when_provider_doesnt_exist() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let provider_id = MassbitId::repeat_byte(0x01);
		assert_noop!(
			DapiStaking::delegator_withdraw_unregistered(Origin::signed(1), provider_id),
			Error::<TestRuntime>::NotOperatedProvider
		);
	})
}

#[test]
fn withdraw_from_unregistered_fail_when_provider_is_still_registered() {
	ExternalityBuilder::build().execute_with(|| {
		initialize_first_block();

		let operator = 1;
		let provider_id = MassbitId::repeat_byte(0x01);
		assert_register(operator, &provider_id, 10);
		assert_noop!(
			DapiStaking::provider_withdraw_unregistered(Origin::signed(1), provider_id),
			Error::<TestRuntime>::NotUnregisteredProvider
		);
	})
}
