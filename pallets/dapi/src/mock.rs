use crate::{self as pallet_dapi, weights};

use frame_support::{construct_runtime, parameter_types, PalletId};
use sp_core::H256;

use frame_support::traits::ConstU32;
use frame_system::EnsureRoot;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use common::MassbitId;

pub(crate) type AccountId = u64;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;
pub(crate) type EraIndex = u32;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

pub(crate) const EXISTENTIAL_DEPOSIT: Balance = 2;
pub(crate) const MIN_PROVIDER_STAKE: Balance = 10;
pub(crate) const PROVIDER_REWARD_PERCENTAGE: u32 = 80;
pub(crate) const MAX_NUMBER_OF_DELEGATORS: u32 = 5;
pub(crate) const MIN_DELEGATOR_STAKE: Balance = 10;
pub(crate) const MAX_UNLOCKING_CHUNKS: u32 = 4;
pub(crate) const UNBONDING_PERIOD: EraIndex = 3;
pub(crate) const MAX_ERA_STAKE_VALUES: u32 = 8;
pub(crate) const BLOCKS_PER_ERA: u32 = 3;

construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Dapi: pallet_dapi::{Pallet, Call, Storage, Config<T>, Event<T>},
		DapiStaking: pallet_dapi_staking::{Pallet, Call, Storage, Event<T>},
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
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const MaxLocks: u32 = 4;
	pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
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
	pub const MinimumPeriod: u64 = 3;
}

impl pallet_timestamp::Config for TestRuntime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinProviderStake: Balance = MIN_PROVIDER_STAKE;
	pub const MaxDelegatorsPerProvider: u32 = MAX_NUMBER_OF_DELEGATORS;
	pub const MinDelegatorStake: Balance = MIN_DELEGATOR_STAKE;
	pub const ProviderRewardsPercentage: Perbill = Perbill::from_percent(PROVIDER_REWARD_PERCENTAGE);
	pub const DapiStakingPalletId: PalletId = PalletId(*b"mokdpstk");
	pub const MaxUnlockingChunks: u32 = MAX_UNLOCKING_CHUNKS;
	pub const UnbondingPeriod: EraIndex = UNBONDING_PERIOD;
	pub const MaxEraStakeValues: u32 = MAX_ERA_STAKE_VALUES;
	pub const DefaultBlocksPerEra: u32 = BLOCKS_PER_ERA;
}

impl pallet_dapi_staking::Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
	type DefaultBlocksPerEra = DefaultBlocksPerEra;
	type ProviderId = MassbitId;
	type ProviderRewardsPercentage = ProviderRewardsPercentage;
	type MinProviderStake = MinProviderStake;
	type MaxDelegatorsPerProvider = MaxDelegatorsPerProvider;
	type MinDelegatorStake = MinDelegatorStake;
	type MaxEraStakeValues = MaxEraStakeValues;
	type UnbondingPeriod = UnbondingPeriod;
	type MaxUnlockingChunks = MaxUnlockingChunks;
	type PalletId = DapiStakingPalletId;
	type WeightInfo = pallet_dapi_staking::weights::SubstrateWeight<TestRuntime>;
}

parameter_types! {
	pub const ProjectDepositPeriod: BlockNumber = 10;
}

impl pallet_dapi::Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
	type DapiStaking = DapiStaking;
	type UpdateOrigin = EnsureRoot<AccountId>;
	type MaxChainIdLength = ConstU32<64>;
	type MassbitId = MassbitId;
	type OnProjectPayment = ();
	type WeightInfo = weights::SubstrateWeight<TestRuntime>;
}

pub struct ExternalityBuilder;

impl ExternalityBuilder {
	pub fn build() -> TestExternalities {
		let mut storage =
			frame_system::GenesisConfig::default().build_storage::<TestRuntime>().unwrap();

		pallet_balances::GenesisConfig::<TestRuntime> {
			balances: vec![
				(1, 9000),
				(2, 800),
				(3, 10000),
				(4, 4900),
				(5, 3800),
				(6, 10),
				(7, 1000),
				(8, 2000),
				(9, 10000),
				(10, 300),
				(11, 400),
				(20, 10),
				(540, EXISTENTIAL_DEPOSIT),
				(1337, 1_000_000_000_000),
			],
		}
		.assimilate_storage(&mut storage)
		.ok();

		let mut ext = TestExternalities::from(storage);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
