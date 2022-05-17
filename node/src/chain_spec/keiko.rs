//! Keiko specifications.

use keiko_runtime::{
	pallet_block_reward, wasm_binary_unwrap, AccountId, AuraConfig, BalancesConfig,
	BlockRewardConfig, DapiConfig, GenesisConfig, GrandpaConfig, SessionConfig, SessionKeys,
	SudoConfig, SystemConfig, ValidatorSetConfig, KEI,
};
use sc_service::ChainType;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::Ss58Codec, sr25519};
use sp_finality_grandpa::AuthorityId as GrandpaId;
use sp_runtime::Perbill;

use super::{get_account_id_from_seed, get_from_seed};

pub type KeikoChainSpec = sc_service::GenericChainSpec<GenesisConfig>;

fn session_keys(aura: AuraId, grandpa: GrandpaId) -> SessionKeys {
	SessionKeys { aura, grandpa }
}

pub fn authority_keys_from_seed(s: &str) -> (AccountId, AuraId, GrandpaId) {
	(
		get_account_id_from_seed::<sr25519::Public>(s),
		get_from_seed::<AuraId>(s),
		get_from_seed::<GrandpaId>(s),
	)
}

pub fn get_chain_spec() -> KeikoChainSpec {
	let mut properties = serde_json::map::Map::new();
	properties.insert("tokenSymbol".into(), "KEI".into());
	properties.insert("tokenDecimals".into(), 18.into());
	KeikoChainSpec::from_genesis(
		"Keiko",
		"keiko",
		ChainType::Live,
		move || {
			make_genesis(
				AccountId::from_ss58check("5GKjNtFNWpvqVCbD4dMXbTXCe31oFdZbLFivMZPvJgRZkjif")
					.unwrap(),
				vec![
					AccountId::from_ss58check("5GKjNtFNWpvqVCbD4dMXbTXCe31oFdZbLFivMZPvJgRZkjif")
						.unwrap(),
					AccountId::from_ss58check("5Gp4aP3CVYWixaM4jq5RKRP1nkG87mmH7913UNLhjeWh28p3")
						.unwrap(),
					AccountId::from_ss58check("5DZFPuBrY2zGPKk9xTV4GvErD86XqFFH3SMjLbrDfrZidsRW")
						.unwrap(),
					AccountId::from_ss58check("5Cg132mksNV7ntfpKrFRJ1fcHrhqtEU9MKBXKV71LGCSwhBz")
						.unwrap(),
					AccountId::from_ss58check("5CyqNYGeb51y4qPJsAVjkeP3BWGUEmEytTWNdMtzqapK2WcY")
						.unwrap(),
				],
				vec![authority_keys_from_seed("Alice")],
				vec![],
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
	root_key: AccountId,
	endowed_accounts: Vec<AccountId>,
	initial_authorities: Vec<(AccountId, AuraId, GrandpaId)>,
	initial_regulators: Vec<AccountId>,
) -> GenesisConfig {
	GenesisConfig {
		system: SystemConfig { code: wasm_binary_unwrap().to_vec() },
		balances: BalancesConfig {
			balances: endowed_accounts.iter().cloned().map(|k| (k, 2_000_000_000 * KEI)).collect(),
		},
		block_reward: BlockRewardConfig {
			// Make sure sum is 100
			reward_config: pallet_block_reward::DistributionConfig {
				providers_percent: Perbill::from_percent(100),
				validators_percent: Perbill::from_percent(0),
			},
		},
		validator_set: ValidatorSetConfig {
			desired_candidates: 200,
			candidacy_bond: 20_000_000 * KEI,
			invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect::<Vec<_>>(),
		},
		session: SessionConfig {
			keys: initial_authorities
				.iter()
				.map(|x| (x.0.clone(), x.0.clone(), session_keys(x.1.clone(), x.2.clone())))
				.collect::<Vec<_>>(),
		},
		aura: AuraConfig { authorities: vec![] },
		grandpa: GrandpaConfig { authorities: vec![] },
		sudo: SudoConfig { key: Some(root_key) },
		dapi: DapiConfig { regulators: initial_regulators.iter().map(|x| x.clone()).collect() },
	}
}
