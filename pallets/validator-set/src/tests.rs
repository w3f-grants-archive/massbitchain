use crate as validator_set;
use crate::{mock::*, CandidateInfo, Error};
use frame_support::{
	assert_noop, assert_ok,
	traits::{Currency, GenesisBuild, OnInitialize},
};
use pallet_balances::Error as BalancesError;
use sp_runtime::traits::BadOrigin;

#[test]
fn basic_setup_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(ValidatorSet::desired_candidates(), 2);
		assert_eq!(ValidatorSet::candidacy_bond(), 10);

		assert!(ValidatorSet::candidates().is_empty());
		assert_eq!(ValidatorSet::invulnerables(), vec![1, 2]);
	});
}

#[test]
fn set_invulnerables() {
	new_test_ext().execute_with(|| {
		let new_invulnerables = vec![1, 2, 3];
		assert_ok!(ValidatorSet::set_invulnerables(
			Origin::signed(RootAccount::get()),
			new_invulnerables.clone()
		));
		assert_eq!(ValidatorSet::invulnerables(), new_invulnerables);

		// cannot set with non-root.
		assert_noop!(
			ValidatorSet::set_invulnerables(Origin::signed(1), new_invulnerables.clone()),
			BadOrigin
		);

		// cannot set invulnerables without associated validator keys
		let invulnerables = vec![7];
		assert_noop!(
			ValidatorSet::set_invulnerables(
				Origin::signed(RootAccount::get()),
				invulnerables.clone()
			),
			Error::<TestRuntime>::ValidatorNotRegistered
		);
	})
}

#[test]
fn set_desired_candidates() {
	new_test_ext().execute_with(|| {
		assert_eq!(ValidatorSet::desired_candidates(), 2);

		assert_ok!(ValidatorSet::set_desired_candidates(Origin::signed(RootAccount::get()), 7));
		assert_eq!(ValidatorSet::desired_candidates(), 7);

		assert_noop!(ValidatorSet::set_desired_candidates(Origin::signed(1), 8), BadOrigin);
	});
}

#[test]
fn set_candidacy_bond() {
	new_test_ext().execute_with(|| {
		assert_eq!(ValidatorSet::candidacy_bond(), 10);

		assert_ok!(ValidatorSet::set_candidacy_bond(Origin::signed(RootAccount::get()), 7));
		assert_eq!(ValidatorSet::candidacy_bond(), 7);

		assert_noop!(ValidatorSet::set_candidacy_bond(Origin::signed(1), 8), BadOrigin);
	});
}

#[test]
fn cannot_register_candidate_if_too_many() {
	new_test_ext().execute_with(|| {
		<crate::DesiredCandidates<TestRuntime>>::put(0);

		assert_noop!(
			ValidatorSet::register_as_candidate(Origin::signed(3)),
			Error::<TestRuntime>::TooManyCandidates,
		);

		<crate::DesiredCandidates<TestRuntime>>::put(1);
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(4)));

		assert_noop!(
			ValidatorSet::register_as_candidate(Origin::signed(5)),
			Error::<TestRuntime>::TooManyCandidates,
		);
	})
}

#[test]
fn cannot_unregister_candidate_if_too_few() {
	new_test_ext().execute_with(|| {
		<crate::DesiredCandidates<TestRuntime>>::put(1);
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(4)));
		assert_noop!(
			ValidatorSet::leave_intent(Origin::signed(4)),
			Error::<TestRuntime>::TooFewCandidates,
		);
	})
}

#[test]
fn cannot_register_as_candidate_if_already_invulnerable() {
	new_test_ext().execute_with(|| {
		assert_eq!(ValidatorSet::invulnerables(), vec![1, 2]);
		assert_noop!(
			ValidatorSet::register_as_candidate(Origin::signed(1)),
			Error::<TestRuntime>::AlreadyInvulnerable,
		);
	})
}

#[test]
fn cannot_register_duplicate_candidate() {
	new_test_ext().execute_with(|| {
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(3)));
		let addition = CandidateInfo { who: 3, deposit: 10 };
		assert_eq!(ValidatorSet::candidates(), vec![addition]);
		assert_eq!(ValidatorSet::last_authored_block(3), 10);
		assert_eq!(Balances::free_balance(3), 90);

		assert_noop!(
			ValidatorSet::register_as_candidate(Origin::signed(3)),
			Error::<TestRuntime>::AlreadyCandidate,
		);
	})
}

#[test]
fn cannot_register_as_candidate_if_insufficient_fund() {
	new_test_ext().execute_with(|| {
		assert_eq!(Balances::free_balance(&3), 100);
		assert_eq!(Balances::free_balance(&33), 0);

		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(3)));

		assert_noop!(
			ValidatorSet::register_as_candidate(Origin::signed(33)),
			BalancesError::<TestRuntime>::InsufficientBalance,
		);
	});
}

#[test]
fn register_as_candidate_successfully() {
	new_test_ext().execute_with(|| {
		assert_eq!(ValidatorSet::desired_candidates(), 2);
		assert_eq!(ValidatorSet::candidacy_bond(), 10);
		assert_eq!(ValidatorSet::candidates(), Vec::new());
		assert_eq!(ValidatorSet::invulnerables(), vec![1, 2]);

		assert_eq!(Balances::free_balance(&3), 100);
		assert_eq!(Balances::free_balance(&4), 100);

		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(3)));
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(4)));

		assert_eq!(Balances::free_balance(&3), 90);
		assert_eq!(Balances::free_balance(&4), 90);

		assert_eq!(ValidatorSet::candidates().len(), 2);
	});
}

#[test]
fn leave_intent() {
	new_test_ext().execute_with(|| {
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(3)));
		assert_eq!(Balances::free_balance(3), 90);

		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(5)));
		assert_eq!(Balances::free_balance(5), 90);

		assert_noop!(
			ValidatorSet::leave_intent(Origin::signed(4)),
			Error::<TestRuntime>::NotCandidate
		);

		assert_ok!(ValidatorSet::leave_intent(Origin::signed(3)));
		assert_eq!(Balances::free_balance(3), 100);
		assert_eq!(ValidatorSet::last_authored_block(3), 0);
	});
}

#[test]
fn authorship_event_handler() {
	new_test_ext().execute_with(|| {
		Balances::make_free_balance_be(&ValidatorSet::account_id(), 105);

		assert_eq!(Balances::free_balance(4), 100);
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(4)));
		Authorship::on_initialize(1);

		let validator = CandidateInfo { who: 4, deposit: 10 };

		assert_eq!(ValidatorSet::candidates(), vec![validator]);
		assert_eq!(ValidatorSet::last_authored_block(4), 0);

		assert_eq!(Balances::free_balance(4), 190);
		assert_eq!(Balances::free_balance(ValidatorSet::account_id()), 5);
	});
}

#[test]
fn session_management_works() {
	new_test_ext().execute_with(|| {
		initialize_to_block(1);

		assert_eq!(SessionChangeBlock::get(), 0);
		assert_eq!(SessionHandlerValidators::get(), vec![1, 2]);

		initialize_to_block(4);

		assert_eq!(SessionChangeBlock::get(), 0);
		assert_eq!(SessionHandlerValidators::get(), vec![1, 2]);

		// add a new validator
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(3)));

		assert_eq!(SessionHandlerValidators::get(), vec![1, 2]);
		assert_eq!(ValidatorSet::candidates().len(), 1);

		initialize_to_block(10);
		assert_eq!(SessionChangeBlock::get(), 10);
		// pallet-session has 1 session delay; current validators are the same.
		assert_eq!(Session::validators(), vec![1, 2]);
		// queued ones are changed, and now we have 3.
		assert_eq!(Session::queued_keys().len(), 3);
		// session handlers (aura, et. al.) cannot see this yet.
		assert_eq!(SessionHandlerValidators::get(), vec![1, 2]);

		initialize_to_block(20);
		assert_eq!(SessionChangeBlock::get(), 20);
		assert_eq!(SessionHandlerValidators::get(), vec![1, 2, 3]);
	});
}

#[test]
fn kick_and_slash_mechanism() {
	new_test_ext().execute_with(|| {
		<crate::SlashDestination<TestRuntime>>::put(5);

		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(3)));
		assert_ok!(ValidatorSet::register_as_candidate(Origin::signed(4)));
		initialize_to_block(10);
		assert_eq!(ValidatorSet::candidates().len(), 2);

		initialize_to_block(20);
		assert_eq!(SessionChangeBlock::get(), 20);
		// 4 authored this block, gets to stay 3 was kicked
		assert_eq!(ValidatorSet::candidates().len(), 1);
		// 3 will be kicked after 1 session delay
		assert_eq!(SessionHandlerValidators::get(), vec![1, 2, 3, 4]);
		let validator = CandidateInfo { who: 4, deposit: 10 };
		assert_eq!(ValidatorSet::candidates(), vec![validator]);
		assert_eq!(ValidatorSet::last_authored_block(4), 20);

		initialize_to_block(30);
		// 3 gets kicked after 1 session delay
		assert_eq!(SessionHandlerValidators::get(), vec![1, 2, 4]);
		// kicked validator gets funds back except slashed 10% (of 10 bond)
		assert_eq!(Balances::free_balance(3), 99);
		assert_eq!(Balances::free_balance(5), 101);
	});
}

#[test]
#[should_panic = "duplicate invulnerables in genesis."]
fn cannot_set_genesis_value_twice() {
	sp_tracing::try_init_simple();
	let mut t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
	let invulnerables = vec![1, 1];

	let validator_set = validator_set::GenesisConfig::<TestRuntime> {
		desired_candidates: 2,
		candidacy_bond: 10,
		invulnerables,
	};
	validator_set.assimilate_storage(&mut t).unwrap();
}
