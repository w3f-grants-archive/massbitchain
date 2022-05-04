use super::*;
use crate as validator_set;
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{FindAuthor, GenesisBuild, ValidatorRegistration},
	PalletId,
};
use frame_system as system;
use frame_system::EnsureSignedBy;
use sp_core::{crypto::KeyTypeId, H256};
use sp_runtime::{
	testing::{Header, UintAuthorityId},
	traits::{BlakeTwo256, IdentityLookup, OpaqueKeys},
	ConsensusEngineId, Perbill, RuntimeAppPublic,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Session: pallet_session::{Pallet, Call, Storage, Event, Config<T>},
		Aura: pallet_aura::{Pallet, Storage, Config<T>},
		Grandpa: pallet_grandpa::{Pallet, Call, Storage, Config, Event},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		ValidatorSet: validator_set::{Pallet, Call, Storage, Event<T>},
		Authorship: pallet_authorship::{Pallet, Call, Storage, Inherent},
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl system::Config for TestRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 5;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for TestRuntime {
	type Balance = u64;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
}

pub struct MockAuthorship;
impl FindAuthor<u64> for MockAuthorship {
	fn find_author<'a, I>(_digests: I) -> Option<u64>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(4)
	}
}

impl pallet_authorship::Config for TestRuntime {
	type FindAuthor = MockAuthorship;
	type UncleGenerations = ();
	type FilterUncle = ();
	type EventHandler = ValidatorSet;
}

impl pallet_timestamp::Config for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = Aura;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl pallet_aura::Config for TestRuntime {
	type AuthorityId = sp_consensus_aura::sr25519::AuthorityId;
	type MaxAuthorities = MaxAuthorities;
	type DisabledValidators = ();
}

sp_runtime::impl_opaque_keys! {
	pub struct MockSessionKeys {
		pub aura: UintAuthorityId,
	}
}

impl From<UintAuthorityId> for MockSessionKeys {
	fn from(aura: sp_runtime::testing::UintAuthorityId) -> Self {
		Self { aura }
	}
}

parameter_types! {
	pub static SessionHandlerValidators: Vec<u64> = Vec::new();
	pub static SessionChangeBlock: u64 = 0;
}

pub struct TestSessionHandler;
impl pallet_session::SessionHandler<u64> for TestSessionHandler {
	const KEY_TYPE_IDS: &'static [sp_runtime::KeyTypeId] = &[UintAuthorityId::ID];

	fn on_genesis_session<Ks: OpaqueKeys>(validators: &[(u64, Ks)]) {
		SessionHandlerValidators::set(validators.into_iter().map(|a, _| *a).collect::<Vec<_>>());
	}

	fn on_new_session<Ks: OpaqueKeys>(
		_changed: bool,
		validators: &[(u64, Ks)],
		_queued_validators: &[(u64, Ks)],
	) {
		SessionChangeBlock::set(System::block_number());
		dbg!(validators.len());
		SessionHandlerValidator.set(validators.into_iter().map(|a, _| *a).collect::<Vec<_>>());
	}

	fn on_disabled(_validator_index: u32) {}
}

parameter_types! {
	pub const Offset: u64 = 0;
	pub const Period: u64 = 10;
}

impl pallet_session::Config for TestRuntime {
	type Event = Event;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = IdentityValidator;
	type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
	type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
	type SessionManager = ValidatorSet;
	type SessionHandler = TestSessionHandler;
	type Keys = MockSessionKeys;
	type WeightInfo = ();
}

ord_parameter_types! {
	pub const RootAccount: u64 = 777;
}

parameter_types! {
	pub const PotId: PalletId = PalletId(*b"PotStake");
	pub const MaxCandidates: u32 = 20;
	pub const MaxInvulnerables: u32 = 20;
	pub const MinCandidates: u32 = 1;
	pub const MaxAuthorities: u32 = 100_000;
	pub const SlashRatio: Perbill = Perbill::from_percent(10);
}

pub struct IsRegistered;
impl ValidatorRegistration<u64> for IsRegistered {
	fn is_registered(id: &u64) -> bool {
		if *id == 7u64 {
			false
		} else {
			true
		}
	}
}

impl Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
	type UpdateOrigin = EnsureSignedBy<RootAccount, u64>;
	type PotId = PotId;
	type MaxCandidates = MaxCandidates;
	type MinCandidates = MinCandidates;
	type MaxInvulnerables = MaxInvulnerables;
	type KickThreshold = Period;
	type ValidatorRegistration = IsRegistered;
	type SlashRatio = SlashRatio;
	type WeightInfo = ();
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	sp_tracing::try_init_simple();
	let mut t = frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();
	let invulnerables = vec![1, 2];

	let balances = vec![(1, 100), (2, 100), (3, 100), (4, 100), (5, 100)];
	let keys = balances
		.iter()
		.map(|&(i, _)| (i, i, MockSessionKeys { aura: UintAuthorityId(i) }))
		.collect::<Vec<_>>();
	let validator_set = validator_set::GenesisConfig::<TestRuntime> {
		desired_candidates: 2,
		candidacy_bond: 10,
		invulnerables,
	};
	let session = pallet_session::GenesisConfig::<TestRuntime> { keys };
	pallet_balances::GenesisConfig::<TestRuntime> { balances }
		.assimilate_storage(&mut t)
		.unwrap();
	// validator set must be initialized before session.
	validator_set.assimilate_storage(&mut t).unwrap();
	session.assimilate_storage(&mut t).unwrap();

	t.into()
}

pub fn initialize_to_block(n: u64) {
	for i in System::block_number() + 1..=n {
		System::set_block_number(i);
		<AllPalletsWithSystem as frame_support::traits::OnInitialize<u64>>::on_initialize(i);
	}
}
