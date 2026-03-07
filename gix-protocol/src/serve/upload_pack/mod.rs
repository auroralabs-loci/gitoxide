///
pub mod want_haves;
pub use want_haves::{parse_haves, parse_wants};

///
pub mod ack;
pub use ack::{write_ack, write_nak, AckStatus};

///
pub mod function;
pub use function::serve_upload_pack_v1;

/// Errors from serving upload-pack.
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to parse client wants/haves")]
    WantHaves(#[from] want_haves::Error),
}
