use crate as pallet_fisherman;
use crate::*;

use codec::Decode;
use frame_support::{parameter_types, sp_io::TestExternalities};
use frame_system::EnsureRoot;
use sp_core::{
	offchain::{testing, OffchainWorkerExt, TransactionPoolExt},
	sr25519,
	sr25519::Signature,
	Pair, Public, H256,
};
use sp_keystore::{testing::KeyStore, KeystoreExt, SyncCryptoStore};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{BlakeTwo256, Extrinsic as ExtrinsicT, IdentifyAccount, IdentityLookup, Verify},
};
use sp_std::{str, vec::Vec};
use std::sync::Arc;

pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;
type AccountPublic = <Signature as Verify>::Signer;

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Membership: pallet_membership::<Instance1>::{Pallet, Call, Storage, Config<T>, Event<T>},
		Fisherman: pallet_fisherman::{Pallet, Call, Storage, Event<T>},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub BlockWeights: frame_system::limits::BlockWeights =
		frame_system::limits::BlockWeights::simple_max(1024);
}

impl frame_system::Config for TestRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

type Extrinsic = TestXt<Call, ()>;
type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

impl frame_system::offchain::SigningTypes for TestRuntime {
	type Public = <Signature as Verify>::Signer;
	type Signature = Signature;
}

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for TestRuntime
where
	Call: From<LocalCall>,
{
	type Extrinsic = Extrinsic;
	type OverarchingCall = Call;
}

parameter_types! {
	pub const MaxLocks: u32 = 4;
	pub const ExistentialDeposit: Balance = 2;
}

impl pallet_balances::Config for TestRuntime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
}

parameter_types! {
	pub const MinimumPeriod: u64 = 1;
}

impl pallet_timestamp::Config for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const FishermanMaxMembers: u32 = 100;
}

impl pallet_membership::Config<pallet_membership::Instance1> for TestRuntime {
	type Event = Event;
	type AddOrigin = EnsureRoot<AccountId>;
	type RemoveOrigin = EnsureRoot<AccountId>;
	type SwapOrigin = EnsureRoot<AccountId>;
	type ResetOrigin = EnsureRoot<AccountId>;
	type PrimeOrigin = EnsureRoot<AccountId>;
	type MembershipInitialized = ();
	type MembershipChanged = ();
	type MaxMembers = FishermanMaxMembers;
	type WeightInfo = ();
}

// pub trait Config: CreateSignedTransaction<Self> + frame_system::Config {}
impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for TestRuntime
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: AppCrypto<Self::Public, Self::Signature>>(
		call: Self::OverarchingCall,
		_public: Self::Public,
		_account: Self::AccountId,
		nonce: Self::Index,
	) -> Option<(Call, <Self::Extrinsic as ExtrinsicT>::SignaturePayload)> {
		Some((call, (nonce, ())))
	}
}

impl pallet_fisherman::Config for TestRuntime {
	type AuthorityId = crypto::TestAuthId;
	type Event = Event;
	type Call = Call;
	type UnixTime = pallet_timestamp::Pallet<TestRuntime>;
	type Members = Membership;
}

#[test]
fn submit_job_result_successfully() {
	const PHRASE: &str =
		"news slush supreme milk chapter athlete soap sausage put clutch what kitten";
	let seed = &format!("{}/fisherman1", PHRASE);
	let keystore = KeyStore::new();
	SyncCryptoStore::sr25519_generate_new(&keystore, pallet_fisherman::KEY_TYPE, Some(seed))
		.unwrap();
	let fisherman = get_account_id_from_seed::<sr25519::Public>(seed);

	let (offchain, offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();

	let mut storage =
		frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

	pallet_membership::GenesisConfig::<TestRuntime, pallet_membership::Instance1> {
		members: vec![fisherman].try_into().expect("convert error!"),
		phantom: Default::default(),
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	pallet_balances::GenesisConfig::<TestRuntime> { balances: vec![(fisherman, 10000)] }
		.assimilate_storage(&mut storage)
		.unwrap();

	let mut ext = TestExternalities::from(storage);
	ext.register_extension(OffchainWorkerExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(KeystoreExt(Arc::new(keystore)));
	mock_post_response(&mut offchain_state.write());
	ext.execute_with(|| {
		let job_id = "1".as_bytes().to_vec();
		let job_name = "RoundTripTime".as_bytes().to_vec();
		let url = "https://api.massbitroute.net/_rtt".as_bytes().to_vec();
		Fisherman::create_job(
			Origin::signed(fisherman),
			vec![],
			job_id,
			job_name,
			vec![],
			vec![],
			vec![],
			vec![],
			vec![],
			vec![],
			vec![],
			url,
			ApiMethod::Post,
			vec![],
			vec![],
		)
		.unwrap();

		Fisherman::execute_jobs(1).unwrap();

		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let Call::Fisherman(crate::Call::submit_job_result {
			job_id: _job_id,
			result: _result,
			is_success,
		}) = tx.call
		{
			assert!(is_success)
		}
	});
}

fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
	TPublic::Pair::from_string(seed, None)
		.expect("static values are valid; qed")
		.public()
}

fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

fn mock_post_response(state: &mut testing::OffchainState) {
	state.expect_request(testing::PendingRequest {
		method: "POST".into(),
		uri: "https://api.massbitroute.net/_rtt".into(),
		response: Some(br#"12345"#.to_vec()),
		sent: true,
		..Default::default()
	});
}
