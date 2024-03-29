use codec::{Decode, Encode};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{Currency, OnFinalize, OnInitialize},
	PalletId,
};
use sp_core::H256;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	Perbill,
};

use crate::{self as pallet_dapi_staking, types::*, weights};

pub(crate) type AccountId = u64;
pub(crate) type BlockNumber = u64;
pub(crate) type Balance = u128;

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
pub(crate) const BLOCK_REWARD: Balance = 123456;

construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
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
	type MaxConsumers = frame_support::traits::ConstU32<16>;
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
	type ProviderId = MockProvider;
	type ProviderRewardsPercentage = ProviderRewardsPercentage;
	type MinProviderStake = MinProviderStake;
	type MaxDelegatorsPerProvider = MaxDelegatorsPerProvider;
	type MinDelegatorStake = MinDelegatorStake;
	type MaxEraStakeValues = MaxEraStakeValues;
	type UnbondingPeriod = UnbondingPeriod;
	type MaxUnlockingChunks = MaxUnlockingChunks;
	type PalletId = DapiStakingPalletId;
	type WeightInfo = weights::SubstrateWeight<TestRuntime>;
}

#[derive(PartialEq, Eq, Copy, Clone, Encode, Decode, Debug, scale_info::TypeInfo)]
pub struct MockProvider([u8; 36]);

impl Default for MockProvider {
	fn default() -> Self {
		MockProvider([1; 36])
	}
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

/// Run to the specified block number
pub fn run_to_block(n: u64) {
	while System::block_number() < n {
		DapiStaking::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		// This is performed outside of dapi staking but we expect it before on_initialize
		payout_block_rewards();
		DapiStaking::on_initialize(System::block_number());
	}
}

/// Run the specified number of blocks
pub fn run_for_blocks(n: u64) {
	run_to_block(System::block_number() + n);
}

/// Advance blocks to the beginning of an era. Function has no effect if era is already passed.
pub fn advance_to_era(n: EraIndex) {
	while DapiStaking::era().current < n {
		run_for_blocks(1);
	}
}

/// Initialize first block.
pub fn initialize_first_block() {
	// This assert prevents method misuse
	assert_eq!(System::block_number(), 1 as BlockNumber);

	// This is performed outside of dapi staking but we expect it before on_initialize
	payout_block_rewards();
	DapiStaking::on_initialize(System::block_number());
	run_to_block(2);
}

/// Payout block rewards to providers & delegators
fn payout_block_rewards() {
	DapiStaking::handle_imbalance(Balances::issue(BLOCK_REWARD.into()));
}
