use gix_error::ErrorExt as _;

use crate::oid;

/// The error returned by [`oid::verify()`].
pub type Error = gix_error::Exn<gix_error::Message>;

impl oid {
    /// Verify that `self` matches the `expected` object ID.
    ///
    /// Returns an [`Error`] containing both object IDs if they differ.
    #[inline]
    pub fn verify(&self, expected: &oid) -> Result<(), Error> {
        if self == expected {
            Ok(())
        } else {
            Err(gix_error::message!("Hash was {}, but should have been {}", self.to_owned(), expected.to_owned()).raise())
        }
    }
}
