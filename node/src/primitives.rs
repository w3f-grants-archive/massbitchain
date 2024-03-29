//! Opaque primitives for different runtimes.

/// An index to a block.
pub type BlockNumber = u32;
/// Header type.
pub type Header = sp_runtime::generic::Header<BlockNumber, sp_runtime::traits::BlakeTwo256>;
/// Block type.
pub type Block = sp_runtime::generic::Block<Header, sp_runtime::OpaqueExtrinsic>;
/// Balance of an account.
pub type Balance = u128;
/// Index of a transaction in the chain.
pub type Nonce = u32;
/// Some way of identifying an account on the chain.
pub type AccountId = sp_runtime::AccountId32;
/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = sp_runtime::MultiSignature;
