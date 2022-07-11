use crate::mock::ExternalityBuilder;
use frame_support::{assert_ok, dispatch::RawOrigin};

use crate::*;
use common::MassbitId;
use mock::*;

const ETH_MAINNET: &str = "eth.mainnet";

fn initialize() {
	Dapi::add_chain_id(RawOrigin::Root.into(), ETH_MAINNET.into()).unwrap();
}

#[test]
fn register_project_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let consumer = 1;
		let project = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_project(
			Origin::signed(consumer),
			project,
			ETH_MAINNET.into(),
			10
		));
	})
}

#[test]
fn deposit_project_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let consumer = 1;
		let project = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_project(
			Origin::signed(consumer),
			project,
			ETH_MAINNET.into(),
			10
		));

		assert_ok!(Dapi::deposit_project(Origin::signed(consumer), project, 10));
	})
}

#[test]
fn submit_project_usage_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let consumer = 1;
		let project = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_project(
			Origin::signed(consumer),
			project,
			ETH_MAINNET.into(),
			10
		));

		assert_ok!(Dapi::submit_project_usage(Origin::signed(10), project, 100));
	})
}

#[test]
fn register_provider_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let operator = 1;
		let provider_id = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_provider(
			Origin::signed(10),
			provider_id,
			ProviderType::Gateway,
			operator,
			ETH_MAINNET.into(),
		));
	})
}

#[test]
fn deposit_provider_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let operator = 1;
		let provider_id = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_provider(
			Origin::signed(10),
			provider_id.clone(),
			ProviderType::Gateway,
			operator,
			ETH_MAINNET.into(),
		));

		assert_ok!(Dapi::deposit_provider(Origin::signed(operator), provider_id.clone(), 100));
	})
}

#[test]
fn unregister_provider_successfully() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let operator = 1;
		let provider_id = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_provider(
			Origin::signed(10),
			provider_id.clone(),
			ProviderType::Gateway,
			operator,
			ETH_MAINNET.into(),
		));

		assert_ok!(Dapi::deposit_provider(Origin::signed(operator), provider_id.clone(), 100));
		assert_ok!(Dapi::unregister_provider(Origin::signed(operator), provider_id.clone()));
	})
}

#[test]
fn report_provider_offence() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		let operator = 1;
		let provider_id = MassbitId::repeat_byte(0x01);
		assert_ok!(Dapi::register_provider(
			Origin::signed(10),
			provider_id.clone(),
			ProviderType::Gateway,
			operator,
			ETH_MAINNET.into(),
		));

		assert_ok!(Dapi::deposit_provider(Origin::signed(operator), provider_id.clone(), 100));
		assert_ok!(Dapi::report_provider_offence(
			Origin::signed(10),
			provider_id.clone(),
			ProviderDeactivateReason::OutOfSync
		));
	})
}

#[test]
fn add_regulator() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		assert_ok!(Dapi::add_regulator(RawOrigin::Root.into(), 2));
	})
}

#[test]
fn remove_regulator() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		assert_ok!(Dapi::remove_regulator(RawOrigin::Root.into(), 10));
	})
}

#[test]
fn add_chain_id() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		assert_ok!(Dapi::add_chain_id(RawOrigin::Root.into(), "dot.mainnet".into()));
	})
}

#[test]
fn remove_chain_id() {
	ExternalityBuilder::build().execute_with(|| {
		initialize();

		assert_ok!(Dapi::remove_chain_id(RawOrigin::Root.into(), "eth.mainnet".into()));
	})
}
