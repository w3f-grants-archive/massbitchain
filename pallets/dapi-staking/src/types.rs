use crate::EraIndex;
use codec::{Decode, Encode, HasCompact};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, Zero},
	RuntimeDebug,
};
use sp_std::{ops::Add, prelude::*};

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum ProviderStatus {
	/// Provider is registered and active.
	Registered,
	/// Provider has been unregistered and is inactive.
	/// Claim for past eras and unbonding is still possible but no additional staking can be done.
	Unregistered(EraIndex),
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct ProviderMetadata<AccountId> {
	pub owner: AccountId,
	pub status: ProviderStatus,
	/// Indicates whether bond were withdrawed by unregistered provider or not.
	pub bond_withdrawn: bool,
}

impl<AccountId> ProviderMetadata<AccountId> {
	pub fn new(owner: AccountId) -> Self {
		Self { owner, status: ProviderStatus::Registered, bond_withdrawn: false }
	}
}

/// A record of rewards allocated for providers and delegators.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct RewardInfo<Balance: HasCompact> {
	#[codec(compact)]
	pub rewards: Balance,
}

/// A record for total rewards and total staked amount for an era.
#[derive(PartialEq, Eq, Clone, Default, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct EraMetadata<Balance: HasCompact> {
	/// Total rewards for an era
	pub rewards: Balance,
	/// Total staked amount in an era
	#[codec(compact)]
	pub staked: Balance,
}

/// Used to split total EraPayout among providers.
/// Each tuple (provider, era) has this structure.
/// This will be used to reward provider and its delegators.
#[derive(Clone, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct ProviderEraMetadata<Balance: HasCompact> {
	/// Provider bond amount.
	#[codec(compact)]
	pub bond: Balance,
	/// Sum of delegators' staked + self.bond
	#[codec(compact)]
	pub total: Balance,
	/// Total number of active delegators.
	#[codec(compact)]
	pub number_of_delegators: u32,
	/// Indicates whether rewards were claimed by provider for this era or not.
	pub provider_reward_claimed: bool,
}

/// Used to represent how much was delegated in a particular era.
/// E.g. `{amount: 1000, era: 5}` means that in era `5`, delegated amount was 1000.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct EraStake<Balance: AtLeast32BitUnsigned + Copy> {
	/// Delegated amount in era
	#[codec(compact)]
	pub amount: Balance,
	/// Delegated era
	#[codec(compact)]
	pub era: EraIndex,
}

impl<Balance: AtLeast32BitUnsigned + Copy> EraStake<Balance> {
	/// Create a new instance of `EraDelegation` with given values
	fn new(amount: Balance, era: EraIndex) -> Self {
		Self { amount, era }
	}
}

/// Used to provide a compact and bounded storage for information about stakes in unclaimed
/// eras.
///
/// # Example
/// For simplicity, the following example will represent `EraStake` using `<era, amount>`
/// notation. Let us assume we have the following vector in `Delegation` struct.
///
/// `[<5, 1000>, <6, 1500>, <8, 2100>, <9, 0>, <11, 500>]`
///
/// This tells us which eras are unclaimed and how much it was staked in each era.
/// The interpretation is the following:
/// 1. In era **5**, staked amount was **1000** (interpreted from `<5, 1000>`)
/// 2. In era **6**, delegator staked additional **500**, increasing total staked amount to **1500**
/// 3. No entry for era **7** exists which means there were no changes from the former entry.
///    This means that in era **7**, staked amount was also **1500**
/// 4. In era **8**, delegator staked an additional **600**, increasing total stake to **2100**
/// 5. In era **9**, delegator unstaked everything from the provider (interpreted from `<9, 0>`)
/// 6. No changes were made in era **10** so we can interpret this same as the previous entry which
/// means **0** staked amount.
/// 7. In era **11**, delegator staked **500** on the provider, making his stake active again after
/// 2 eras of inactivity.
///
/// **NOTE:** It is important to understand that delegator **DID NOT** claim any rewards during this
/// period.
#[derive(Encode, Decode, Clone, Default, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct Delegation<Balance: AtLeast32BitUnsigned + Copy> {
	// Size of this list would be limited by a configurable constant
	stakes: Vec<EraStake<Balance>>,
}

impl<Balance: AtLeast32BitUnsigned + Copy> Delegation<Balance> {
	/// `true` if no active delegations and unclaimed eras exist, `false` otherwise
	pub fn is_empty(&self) -> bool {
		self.stakes.is_empty()
	}

	/// Number of `EraDelegation` chunks
	pub fn len(&self) -> u32 {
		self.stakes.len() as u32
	}

	/// Stake some amount in the specified era.
	///
	/// Delegator should ensure that given era is either equal or greater than the
	/// latest available era in the delegation info.
	///
	/// # Example
	///
	/// The following example demonstrates how internal vector changes when `stake` is called:
	///
	/// `delegations: [<5, 1000>, <7, 1300>]`
	/// * `stake(7, 100)` will result in `[<5, 1000>, <7, 1400>]`
	/// * `stake(9, 200)` will result in `[<5, 1000>, <7, 1400>, <9, 1600>]`
	pub fn stake(&mut self, current_era: EraIndex, amount: Balance) -> Result<(), &str> {
		if let Some(delegation) = self.stakes.last_mut() {
			if delegation.era > current_era {
				return Err("Unexpected era".into());
			}

			let new_stake_amount = delegation.amount.saturating_add(amount);
			if current_era == delegation.era {
				*delegation = EraStake::new(new_stake_amount, current_era)
			} else {
				self.stakes.push(EraStake::new(new_stake_amount, current_era))
			}
		} else {
			self.stakes.push(EraStake::new(amount, current_era))
		}

		Ok(())
	}

	/// Unstake some amount in the specified era.
	///
	/// Delegator should ensure that given era is either equal or greater than the
	/// latest available era in the delegation info.
	///
	/// # Example 1
	///
	/// `delegations: [<5, 1000>, <7, 1300>]`
	/// * `unstake(7, 100)` will result in `[<5, 1000>, <7, 1200>]`
	/// * `unstake(9, 400)` will result in `[<5, 1000>, <7, 1200>, <9, 800>]`
	/// * `unstake(10, 800)` will result in `[<5, 1000>, <7, 1200>, <9, 800>, <0, 10>]`
	///
	/// # Example 2
	///
	/// `delegations: [<5, 1000>]`
	/// * `unstake(1000, 0)` will result in `[]`
	///
	/// Note that if no unclaimed eras remain, vector will be cleared.
	pub fn unstake(&mut self, current_era: EraIndex, amount: Balance) -> Result<(), &str> {
		if let Some(delegation) = self.stakes.last_mut() {
			if delegation.era > current_era {
				return Err("Unexpected era".into());
			}

			let new_stake_amount = delegation.amount.saturating_sub(amount);
			if current_era == delegation.era {
				*delegation = EraStake::new(new_stake_amount, current_era)
			} else {
				self.stakes.push(EraStake::new(new_stake_amount, current_era))
			}

			// Removes unstaked values if they're no longer valid for comprehension
			if !self.stakes.is_empty() && self.stakes[0].amount.is_zero() {
				self.stakes.remove(0);
			}
		}

		Ok(())
	}

	/// `Claims` the oldest era available for claiming.
	/// In case valid era exists, returns `(claim era, staked amount)` tuple.
	/// If no valid era exists, returns `(0, 0)` tuple.
	///
	/// # Example
	///
	/// The following example will demonstrate how the internal vec changes when `claim` is called
	/// consecutively.
	///
	/// `delegations: [<5, 1000>, <7, 1300>, <8, 0>, <15, 3000>]`
	///
	/// 1. `claim()` will return `(5, 1000)`
	///     Internal vector is modified to `[<6, 1000>, <7, 1300>, <8, 0>, <15, 3000>]`
	///
	/// 2. `claim()` will return `(6, 1000)`.
	///    Internal vector is modified to `[<7, 1300>, <8, 0>, <15, 3000>]`
	///
	/// 3. `claim()` will return `(7, 1300)`.
	///    Internal vector is modified to `[<15, 3000>]`
	///    Note that `0` bonded period is discarded since nothing can be claimed there.
	///
	/// 4. `claim()` will return `(15, 3000)`.
	///    Internal vector is modified to `[16, 3000]`
	///
	/// Repeated calls would continue to modify vector following the same rule as in *4.*
	pub fn claim(&mut self) -> (EraIndex, Balance) {
		if let Some(delegation) = self.stakes.first() {
			let delegation = *delegation;

			if self.stakes.len() == 1 || self.stakes[1].era > delegation.era + 1 {
				self.stakes[0] =
					EraStake { amount: delegation.amount, era: delegation.era.saturating_add(1) }
			} else {
				// in case: self.delegations[1].era == delegation.era + 1
				self.stakes.remove(0);
			}

			// Removes unstaked values if they're no longer valid for comprehension
			if !self.stakes.is_empty() && self.stakes[0].amount.is_zero() {
				self.stakes.remove(0);
			}

			(delegation.era, delegation.amount)
		} else {
			(0, Zero::zero())
		}
	}

	/// Latest staked value.
	/// E.g. if delegator is fully unstaked, this will return `Zero`.
	/// Otherwise returns a non-zero balance.
	pub fn latest_staked_value(&self) -> Balance {
		self.stakes.last().map_or(Zero::zero(), |x| x.amount)
	}
}

/// Represents an balance amount undergoing the unbonding process.
/// Since unbonding takes time, it's important to keep track of when and how much was unbonded.
#[derive(Clone, Copy, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct UnlockingChunk<Balance> {
	/// Amount being unlocked
	#[codec(compact)]
	pub amount: Balance,
	/// Era in which the amount can be withdrawn.
	#[codec(compact)]
	pub unlock_era: EraIndex,
}

impl<Balance> UnlockingChunk<Balance>
where
	Balance: Add<Output = Balance> + Copy,
{
	// Adds the specified amount to this chunk
	pub fn add_amount(&mut self, amount: Balance) {
		self.amount = self.amount + amount
	}
}

/// Contains unlocking chunks which provides various utility methods to help with unbonding
/// handling.
#[derive(Clone, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct UnbondingMetadata<Balance: AtLeast32BitUnsigned + Default + Copy> {
	// Vector of unlocking chunks. Sorted in ascending order in respect to unlock_era.
	unlocking_chunks: Vec<UnlockingChunk<Balance>>,
}

impl<Balance> UnbondingMetadata<Balance>
where
	Balance: AtLeast32BitUnsigned + Default + Copy,
{
	/// Returns total number of unlocking chunks.
	pub fn len(&self) -> u32 {
		self.unlocking_chunks.len() as u32
	}

	/// True if no unlocking chunks exist, false otherwise.
	pub fn is_empty(&self) -> bool {
		self.unlocking_chunks.is_empty()
	}

	/// Returns sum of all unlocking chunks.
	pub fn sum(&self) -> Balance {
		self.unlocking_chunks
			.iter()
			.map(|chunk| chunk.amount)
			.reduce(|c1, c2| c1 + c2)
			.unwrap_or_default()
	}

	/// Adds a new unlocking chunk to the vector, preserving the unlock_era based ordering.
	pub fn add(&mut self, chunk: UnlockingChunk<Balance>) {
		// It is possible that the unbonding period changes so we need to account for that
		match self.unlocking_chunks.binary_search_by(|x| x.unlock_era.cmp(&chunk.unlock_era)) {
			// Merge with existing chunk if unlock_eras match
			Ok(pos) => self.unlocking_chunks[pos].add_amount(chunk.amount),
			// Otherwise insert where it should go. Note that this will in almost all cases return
			// the last index.
			Err(pos) => self.unlocking_chunks.insert(pos, chunk),
		}
	}

	/// Partitions the unlocking chunks into two groups:
	///
	/// First group includes all chunks which have unlock era lesser or equal to the specified era.
	/// Second group includes all the rest.
	///
	/// Order of chunks is preserved in the two new structs.
	pub fn partition(self, era: EraIndex) -> (Self, Self) {
		let (matching_chunks, other_chunks): (
			Vec<UnlockingChunk<Balance>>,
			Vec<UnlockingChunk<Balance>>,
		) = self.unlocking_chunks.iter().partition(|chunk| chunk.unlock_era <= era);

		(Self { unlocking_chunks: matching_chunks }, Self { unlocking_chunks: other_chunks })
	}
}

/// Contains information about account's unbonding balances.
#[derive(Clone, PartialEq, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct AccountLedger<Balance: AtLeast32BitUnsigned + Default + Copy> {
	/// Information about unbonding chunks.
	pub unbonding_info: UnbondingMetadata<Balance>,
}

impl<Balance: AtLeast32BitUnsigned + Default + Copy> AccountLedger<Balance> {
	/// `true` if ledger is empty (no locked funds, no unbonding chunks), `false` otherwise.
	pub fn is_empty(&self) -> bool {
		self.unbonding_info.is_empty()
	}
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
/// The current era index and transition information
pub struct EraInfo<BlockNumber> {
	/// Current era index
	pub current: EraIndex,
	/// The starting block of the current era
	pub starting_block: BlockNumber,
	/// The length of the current era in number of blocks
	pub length: u32,
}
impl<
		B: Copy + sp_std::ops::Add<Output = B> + sp_std::ops::Sub<Output = B> + From<u32> + PartialOrd,
	> EraInfo<B>
{
	pub fn new(current: EraIndex, first: B, length: u32) -> EraInfo<B> {
		EraInfo { current, starting_block: first, length }
	}

	/// Check if the era should be updated
	pub fn should_update(&self, now: B) -> bool {
		now - self.starting_block >= self.length.into()
	}

	/// New era
	pub fn update(&mut self, now: B) {
		self.current = self.current.saturating_add(1u32);
		self.starting_block = now;
	}
}
impl<
		B: Copy + sp_std::ops::Add<Output = B> + sp_std::ops::Sub<Output = B> + From<u32> + PartialOrd,
	> Default for EraInfo<B>
{
	fn default() -> EraInfo<B> {
		EraInfo::new(1u32, 1u32.into(), 20u32)
	}
}
