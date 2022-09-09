#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	dispatch::DispatchResultWithPostInfo,
	log,
	pallet_prelude::*,
	traits::{SortedMembers, UnixTime},
	IterableStorageMap,
};
use frame_system::{
	self as system,
	offchain::{AppCrypto, CreateSignedTransaction, ForAll, SendSignedTransaction, Signer},
	pallet_prelude::*,
};
use scale_info::TypeInfo;
use serde::{Deserialize, Deserializer, Serialize};
use sp_core::crypto::KeyTypeId;
use sp_runtime::offchain::{http, Duration};
use sp_std::{convert::TryInto, prelude::*, str, vec, vec::Vec};

pub use pallet::*;

#[cfg(test)]
mod tests;

/// Defines application identifier for crypto keys of this module.
///
/// Every module that deals with signatures needs to declare its unique identifier for its crypto
/// keys. When offchain worker is signing transactions it's going to request keys of type
/// `KeyTypeId` from the keystore and use the ones it finds to sign the transaction. The keys can be
/// inserted manually via RPC (see `author_insertKey`).
pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"ocwr");

/// Based on the above `KeyTypeId` we need to generate a pallet-specific crypto type wrappers. We
/// can use from supported crypto kinds (`sr25519`, `ed25519` and `ecdsa`) and augment the types
/// with this pallet-specific identifier.
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

	pub type JobId = Vec<u8>;

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

		/// Fisherman membership.
		type Members: SortedMembers<Self::AccountId>;
	}

	#[pallet::storage]
	#[pallet::getter(fn jobs)]
	pub type Jobs<T: Config> = StorageMap<_, Blake2_128Concat, JobId, Job, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn job_results)]
	pub type JobResults<T: Config> = StorageMap<_, Blake2_128Concat, JobId, JobResult, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Job does not exist
		JobNotExist,
		/// Sender does not have permission
		NoPermission,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New job is created.
		NewJob { submitter: T::AccountId, job_id: JobId },
		/// New job result is submitted by operators.
		NewJobResult { job_id: JobId, job: Job, job_result: JobResult },
		/// Job is removed.
		JobRemoved { job_id: JobId },
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

			let res = Self::execute_jobs(block_number);
			if let Err(e) = res {
				log::error!("Error: {}", e);
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		pub fn create_job(
			origin: OriginFor<T>,
			plan_id: Vec<u8>,
			job_id: JobId,
			job_name: Vec<u8>,
			provider_id: Vec<u8>,
			provider_type: Vec<u8>,
			phase: Vec<u8>,
			chain: Vec<u8>,
			network: Vec<u8>,
			response_type: Vec<u8>,
			response_values: Vec<u8>,
			url: Vec<u8>,
			method: ApiMethod,
			headers: Vec<(Vec<u8>, Vec<u8>)>,
			payload: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let submitter = ensure_signed(origin)?;
			ensure!(T::Members::contains(&submitter), Error::<T>::NoPermission);

			let job = Job {
				plan_id,
				job_name,
				provider_id,
				provider_type,
				phase,
				chain,
				network,
				response_type,
				response_values,
				url,
				method,
				headers,
				payload,
			};
			Jobs::<T>::insert(&job_id, job.clone());
			Self::deposit_event(Event::NewJob { submitter, job_id });
			Ok(Pays::No.into())
		}

		#[pallet::weight(10_000)]
		pub fn submit_job_result(
			origin: OriginFor<T>,
			job_id: Vec<u8>,
			result: Vec<u8>,
			is_success: bool,
		) -> DispatchResultWithPostInfo {
			let submitter = ensure_signed(origin)?;
			ensure!(T::Members::contains(&submitter), Error::<T>::NoPermission);

			let job = Jobs::<T>::get(&job_id).ok_or(Error::<T>::JobNotExist)?;
			let now = T::UnixTime::now().as_millis();
			let job_result = JobResult { result, timestamp: now, is_success };
			JobResults::<T>::insert(&job_id, job_result.clone());

			Self::deposit_event(Event::NewJobResult { job_id, job, job_result });
			Ok(Pays::No.into())
		}

		#[pallet::weight(10_000)]
		pub fn clear_job(origin: OriginFor<T>, job_id: Vec<u8>) -> DispatchResultWithPostInfo {
			let submitter = ensure_signed(origin)?;
			ensure!(T::Members::contains(&submitter), Error::<T>::NoPermission);

			let job_exists = Jobs::<T>::contains_key(&job_id);
			if job_exists {
				<Jobs<T>>::remove(&job_id);
				Self::deposit_event(Event::JobRemoved { job_id });
			}
			Ok(Pays::No.into())
		}

		/// TODO: Remove this
		#[pallet::weight(10_000)]
		pub fn clear_all_jobs(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let _ = ensure_root(origin)?;
			let _ = <Jobs<T>>::clear(1000, None);
			Ok(Pays::No.into())
		}
	}
}

#[derive(Clone, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Job {
	plan_id: Vec<u8>,
	job_name: Vec<u8>,
	provider_id: Vec<u8>,
	provider_type: Vec<u8>,
	phase: Vec<u8>,
	chain: Vec<u8>,
	network: Vec<u8>,
	response_type: Vec<u8>,
	response_values: Vec<u8>,
	url: Vec<u8>,
	method: ApiMethod,
	headers: Vec<(Vec<u8>, Vec<u8>)>,
	payload: Vec<u8>,
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
	is_success: bool,
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
	fn execute_jobs(_block_number: T::BlockNumber) -> Result<(), &'static str> {
		let signer = Signer::<T, T::AuthorityId>::all_accounts();
		if !signer.can_sign() {
			return Err(
				"No local accounts available. Consider adding one via `author_insertKey` RPC.",
			)?
		}

		for (job_id, job) in <Jobs<T> as IterableStorageMap<_, _>>::iter() {
			let response: Vec<u8>;
			let mut is_success = true;
			match job.method {
				ApiMethod::Get => {
					response = Self::send_http_get_request(job.url.clone()).unwrap_or_else(|_| {
						is_success = false;
						"Failed to send request".as_bytes().to_vec()
					});
				},
				ApiMethod::Post => {
					response = Self::send_http_post_request(
						job.url.clone(),
						job.headers.clone(),
						job.payload.clone(),
					)
					.unwrap_or_else(|_| {
						is_success = false;
						"Failed to send request".as_bytes().to_vec()
					});
				},
			}

			if !is_success {
				Self::send_job_result(&signer, &job_id, &response, is_success);
				return Ok(())
			}

			match str::from_utf8(&job.job_name) {
				Ok("LatestBlock") => {
					let res: LatestBlockResponse = serde_json::from_slice(&response)
						.expect("Response JSON was not well-formatted");
					let data = serde_json::to_vec(&res.result).unwrap();
					Self::send_job_result(&signer, &job_id, &data, is_success);
				},
				Ok("RoundTripTime") => {
					log::info!("{}", str::from_utf8(&response).unwrap());
					Self::send_job_result(&signer, &job_id, &response, is_success);
				},
				_ => (),
			}
		}
		Ok(())
	}

	fn send_job_result(
		signer: &Signer<T, <T as Config>::AuthorityId, ForAll>,
		job_id: &Vec<u8>,
		result: &Vec<u8>,
		is_success: bool,
	) {
		let results = signer.send_signed_transaction(|_account| Call::submit_job_result {
			job_id: job_id.clone(),
			result: result.clone(),
			is_success,
		});
		for (acc, res) in &results {
			match res {
				Ok(()) => log::info!("[{:?}] Submitted data", acc.id),
				Err(e) => log::error!("[{:?}] Failed to submit transaction: {:?}", acc.id, e),
			}
		}
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

	fn send_http_post_request(
		url: Vec<u8>,
		headers: Vec<(Vec<u8>, Vec<u8>)>,
		payload: Vec<u8>,
	) -> Result<Vec<u8>, http::Error> {
		let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(10_000));
		let mut request = http::Request::post(str::from_utf8(&url).unwrap(), vec![payload.clone()]);
		for (key, val) in headers.iter() {
			let key_str = sp_std::str::from_utf8(&key).unwrap_or_default();
			let val_str = sp_std::str::from_utf8(&val).unwrap_or_default();
			request = request.add_header(key_str, val_str);
		}
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
}