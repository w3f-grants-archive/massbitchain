#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResultWithPostInfo,
	log,
	pallet_prelude::*,
	traits::{ChangeMembers, Currency, EstimateCallFee, Get, SortedMembers, UnixTime},
	IterableStorageDoubleMap, IterableStorageMap,
};
use frame_system::{
	self as system,
	offchain::{
		AppCrypto, CreateSignedTransaction, SendSignedTransaction, Signer, SubmitTransaction,
	},
	pallet_prelude::*,
	Config as SystemConfig,
};
use hex::ToHex;
use lite_json::{
	json::{JsonValue, NumberValue},
	Serialize as JsonSerialize,
};
use orml_utilities::OrderedSet;
use scale_info::TypeInfo;
use serde::{Deserialize, Deserializer, Serialize};
use sp_core::crypto::KeyTypeId;
use sp_runtime::{
	offchain::{
		http,
		storage::{MutateStorageError, StorageRetrievalError, StorageValueRef},
		Duration,
	},
	traits::{Hash, UniqueSaturatedInto, Zero},
};
use sp_std::{
	borrow::ToOwned,
	convert::{TryFrom, TryInto},
	prelude::*,
	str, vec,
	vec::Vec,
};

pub use pallet::*;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for
/// its crypto keys.
/// When offchain worker is signing transactions it's going to request keys of type
/// `KeyTypeId` from the keystore and use the ones it finds to sign the transaction.
/// The keys can be inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ocwr");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers.
/// We can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment
/// the types with this pallet-specific identifier.
pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		traits::Verify,
		MultiSignature, MultiSigner,
	};
	use sp_std::convert::TryFrom;

	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericPublic = sp_core::sr25519::Public;
		type GenericSignature = sp_core::sr25519::Signature;
	}
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature>
		for TestAuthId
	{
		type RuntimeAppPublic = Public;
		type GenericPublic = sp_core::sr25519::Public;
		type GenericSignature = sp_core::sr25519::Signature;
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	#[pallet::generate_store(trait Store)]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: CreateSignedTransaction<Call<Self>> + frame_system::Config {
		/// The identifier type of an offchain worker.
		type AuthorityId: AppCrypto<Self::Public, Self::Signature>;

		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The overarching dispatch call type.
		type Call: From<Call<Self>> + Encode;

		type UnixTime: UnixTime;

		/// A configuration for base priority of unsigned transactions.
		///
		/// This is exposed so that it can be tuned for particular runtime, when
		/// multiple pallets send unsigned transactions.
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;
	}

	#[pallet::storage]
	#[pallet::getter(fn jobs)]
	pub type Jobs<T: Config> =
		StorageMap<_, Blake2_128Concat, Vec<u8>, Job<T::AccountId>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn job_results)]
	pub type JobResults<T: Config> =
		StorageMap<_, Blake2_128Concat, Vec<u8>, JobResult, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		JobNotExist,
		/// DataRequest Fields is too large to store on-chain.
		TooLarge,
		/// Sender does not have permission
		NoPermission,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New job is submitted.
		NewJob {
			operator: T::AccountId,
			job: Job<T::AccountId>,
		},
		NewJobResult {
			job: Job<T::AccountId>,
			job_result: JobResult,
		},
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			// Note that having logs compiled to WASM may cause the size of the blob to increase
			// significantly. You can use `RuntimeDebug` custom derive to hide details of the types
			// in WASM. The `sp-api` crate also provides a feature `disable-logging` to disable
			// all logging and thus, remove any logging from the WASM.
			let parent_hash = <system::Pallet<T>>::block_hash(block_number - 1u32.into());
			log::debug!("Current block: {:?} (parent hash: {:?})", block_number, parent_hash);

			let res = Self::fetch_data_and_send_raw_unsigned(block_number);
			if let Err(e) = res {
				log::error!("Error: {}", e);
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		/// Validate unsigned call to this module.
		///
		/// By default unsigned transactions are disallowed, but implementing the validator here we
		/// make sure that some particular calls (the ones produced by offchain worker) are being
		/// whitelisted and marked as valid.
		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			if let Call::submit_job_result { .. } = call {
				Self::validate_transaction()
			} else {
				InvalidTransaction::Call.into()
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_job(
			origin: OriginFor<T>,
			plan_id: Vec<u8>,
			job_id: Vec<u8>,
			job_name: Vec<u8>,
			worker_id: T::AccountId,
			provider_id: Vec<u8>,
			provider_type: Vec<u8>,
			phase: Vec<u8>,
			chain: Vec<u8>,
			network: Vec<u8>,
			response_type: Vec<u8>,
			response_values: Vec<u8>,
			url: Vec<u8>,
			method: ApiMethod,
			payload: Option<Vec<u8>>,
		) -> DispatchResultWithPostInfo {
			let operator = ensure_signed(origin)?;
			// ensure!(T::Members::contains(&operator), Error::<T>::NoPermission);
			let job = Job {
				plan_id,
				job_name,
				worker_id,
				provider_id,
				provider_type,
				phase,
				chain,
				network,
				response_type,
				response_values,
				url,
				method,
				payload,
			};
			Jobs::<T>::insert(&job_id, job.clone());
			Self::deposit_event(Event::NewJob { operator, job });
			Ok(Pays::No.into())
		}

		#[pallet::weight(10_000)]
		pub fn submit_job_result(
			origin: OriginFor<T>,
			job_id: Vec<u8>,
			result: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			ensure_none(origin)?;

			let job = Jobs::<T>::get(&job_id).ok_or(Error::<T>::JobNotExist)?;

			let now = T::UnixTime::now().as_millis();
			let job_result = JobResult { result, timestamp: now };
			JobResults::<T>::insert(&job_id, job_result.clone());

			Self::deposit_event(Event::NewJobResult { job, job_result });
			Ok(Pays::No.into())
		}
	}
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Job<AccountId> {
	plan_id: Vec<u8>,
	job_name: Vec<u8>,
	worker_id: AccountId,
	provider_id: Vec<u8>,
	provider_type: Vec<u8>,
	phase: Vec<u8>,
	chain: Vec<u8>,
	network: Vec<u8>,
	response_type: Vec<u8>,
	response_values: Vec<u8>,
	url: Vec<u8>,
	method: ApiMethod,
	payload: Option<Vec<u8>>,
}

#[derive(Copy, Clone, PartialEq, Eq, Encode, Decode, RuntimeDebug, TypeInfo)]
#[cfg_attr(feature = "std", derive(serde::Serialize, serde::Deserialize))]
pub enum ApiMethod {
	Get,
	Post,
}

#[derive(Clone, PartialEq, Eq, Default, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct JobResult {
	result: Vec<u8>,
	timestamp: u128,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct LatestBlockResponse {
	result: LatestBlock,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatestBlock {
	#[serde(deserialize_with = "de_string_to_bytes")]
	hash: Vec<u8>,
	#[serde(deserialize_with = "de_string_to_bytes")]
	number: Vec<u8>,
}

pub fn de_string_to_bytes<'de, D>(de: D) -> Result<Vec<u8>, D::Error>
where
	D: Deserializer<'de>,
{
	let s: &str = Deserialize::deserialize(de)?;
	Ok(s.as_bytes().to_vec())
}

impl<T: Config> Pallet<T> {
	fn fetch_data_and_send_raw_unsigned(block_number: T::BlockNumber) -> Result<(), &'static str> {
		for (job_id, job) in <Jobs<T> as IterableStorageMap<_, _>>::iter() {
			let mut response = vec![];
			match job.method {
				ApiMethod::Get => {
					response = Self::send_http_get_request(job.url.clone())
						.unwrap_or("Failed to send request".as_bytes().to_vec());
				},
				ApiMethod::Post => {
					response = Self::send_http_post_request(
						job.url.clone(),
						job.payload.clone().unwrap_or_default(),
					)
					.unwrap_or("Failed to send request".as_bytes().to_vec());
				},
			}

			match str::from_utf8(&job.job_name.clone()) {
				Ok("LatestBlock") => {
					let res: LatestBlockResponse = serde_json::from_slice(&response)
						.expect("Response JSON was not well-formatted");
					let data = serde_json::to_vec(&res.result).unwrap();
					let result = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
						Call::submit_job_result { job_id, result: data }.into(),
					);
					if let Err(e) = result {
						log::error!("Error submitting unsigned transaction: {:?}", e);
					}
				},
				Ok("RoundTripTime") => {
					let result = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(
						Call::submit_job_result { job_id, result: response }.into(),
					);
					if let Err(e) = result {
						log::error!("Error submitting unsigned transaction: {:?}", e);
					}
				},
				_ => (),
			}
		}
		Ok(())
	}

	fn send_http_get_request(url: Vec<u8>) -> Result<Vec<u8>, http::Error> {
		let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(10_000));
		let request = http::Request::get(str::from_utf8(&url).unwrap());
		let pending = request.deadline(deadline).send().map_err(|_| http::Error::IoError)?;
		let response = pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;
		if response.code != 200 {
			log::info!("Unexpected status code: {}", response.code);
			return Err(http::Error::Unknown)
		}

		let body = response.body().collect::<Vec<u8>>();
		let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
			log::info!("No UTF8 body");
			http::Error::Unknown
		})?;

		Ok(body_str.as_bytes().to_vec())
	}

	fn send_http_post_request(url: Vec<u8>, payload: Vec<u8>) -> Result<Vec<u8>, http::Error> {
		let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(10_000));
		let request = http::Request::post(str::from_utf8(&url).unwrap(), vec![payload.clone()])
			.add_header("content-type", "application/json");
		let pending = request.deadline(deadline).send().map_err(|_| http::Error::IoError)?;
		let response = pending.try_wait(deadline).map_err(|_| http::Error::DeadlineReached)??;
		if response.code != 200 {
			log::info!("Unexpected status code: {}", response.code);
			return Err(http::Error::Unknown)
		}

		let body = response.body().collect::<Vec<u8>>();
		let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
			log::info!("No UTF8 body");
			http::Error::Unknown
		})?;

		Ok(body_str.as_bytes().to_vec())
	}

	fn validate_transaction() -> TransactionValidity {
		ValidTransaction::with_tag_prefix("MassbitOCW")
			.priority(T::UnsignedPriority::get())
			.longevity(5)
			.propagate(true)
			.build()
	}
}
