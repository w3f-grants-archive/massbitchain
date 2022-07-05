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
