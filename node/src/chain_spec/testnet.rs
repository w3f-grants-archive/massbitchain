//! Testnet specifications.

use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_service::ChainType;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{sr25519, Pair, Public};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::{
	traits::{IdentifyAccount, Verify},
	Perbill,
};
use testnet_runtime::{
	pallet_block_reward, wasm_binary_unwrap, AccountId, AuraConfig, BalancesConfig,
	BlockRewardConfig, DapiConfig, GenesisConfig, GrandpaConfig, ImOnlineConfig, SessionConfig,
	SessionKeys, Signature, SudoConfig, SystemConfig, ValidatorSetConfig,
};

use super::get_from_seed;

pub type TestnetChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

fn session_keys(aura: AuraId, grandpa: GrandpaId, im_online: ImOnlineId) -> SessionKeys {
	SessionKeys { aura, grandpa, im_online }
}

pub fn authority_keys_from_seed(s: &str) -> (AccountId, AuraId, GrandpaId, ImOnlineId) {
	(
		get_account_id_from_seed::<sr25519::Public>(s),
		get_from_seed::<AuraId>(s),
		get_from_seed::<GrandpaId>(s),
		get_from_seed::<ImOnlineId>(s),
	)
}

pub fn get_chain_spec() -> TestnetChainSpec {
	let mut properties = serde_json::map::Map::new();
	properties.insert("tokenSymbol".into(), "MBTT".into());
	properties.insert("tokenDecimals".into(), 18.into());
	TestnetChainSpec::from_genesis(
		"Testnet",
		"testnet",
		ChainType::Development,
		move || {
			make_genesis(
				// Initial PoA authorities
				vec![authority_keys_from_seed("Alice")],
				// Sudo account
				get_account_id_from_seed::<sr25519::Public>("Alice"),
				// Pre-funded accounts
				vec![
					get_account_id_from_seed::<sr25519::Public>("Alice"),
					get_account_id_from_seed::<sr25519::Public>("Bob"),
					get_account_id_from_seed::<sr25519::Public>("Dave"),
					get_account_id_from_seed::<sr25519::Public>("Charlie"),
					get_account_id_from_seed::<sr25519::Public>("Eve"),
					get_account_id_from_seed::<sr25519::Public>("Ferdie"),
				],
				vec![get_account_id_from_seed::<sr25519::Public>("Ferdie")],
			)
		},
		vec![],
		None,
		None,
		None,
		Some(properties),
		None,
	)
}

fn make_genesis(
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId, ImOnlineId)>,
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	initial_regulators: Vec<AccountId>,
) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig { code: wasm_binary_unwrap().to_vec() },
		balances: BalancesConfig {
			balances: endowed_accounts
				.iter()
				.cloned()
				.map(|k| (k, 100_000_000_000_000_000_000_000))
				.collect(),
		},
		block_reward: BlockRewardConfig {
			// Make sure sum is 100
			reward_config: pallet_block_reward::DistributionConfig {
				providers_percent: Perbill::from_percent(100),
				validators_percent: Perbill::zero(),
			},
		},
		validator_set: ValidatorSetConfig {
			initial_validators: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| {
					(x.0.clone(), x.0.clone(), session_keys(x.1.clone(), x.2.clone(), x.3.clone()))
				})
				.collect::<Vec<_>>(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		im_online: ImOnlineConfig { keys: vec![] },
		sudo: SudoConfig { key: Some(root_key) },
		dapi: DapiConfig { regulators: initial_regulators.iter().map(|x| x.clone()).collect() },
	}
}

type AccountPublic = <Signature as Verify>::Signer;

/// Generate an account ID from seed.
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
	AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
	AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}
