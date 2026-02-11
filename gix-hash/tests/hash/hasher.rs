use gix_hash::{Hasher, ObjectId};
use gix_testtools::size_ok;

#[test]
fn size_of_hasher_sha1_only() {
    let actual = std::mem::size_of::<Hasher>();
    let expected = 824;
    assert!(
        size_ok(actual, expected),
        "The size of this type may be relevant when hashing millions of objects, and shouldn't\
        change unnoticed: {actual} <~ {expected}\
        (The DetectionState alone clocked in at 724 bytes when last examined.)"
    );
}

#[test]
#[cfg(all(feature = "sha256", feature = "sha1"))]
fn size_of_hasher_sha1_and_sha256() {
    let actual = std::mem::size_of::<Hasher>();
    let expected = 824;
    assert!(
        size_ok(actual, expected),
        "The size of this type may be relevant when hashing millions of objects, and shouldn't\
        change unnoticed: {actual} <~ {expected}\
        (The DetectionState alone clocked in at 724 bytes when last examined.)"
    );
}

#[test]
#[cfg(all(not(feature = "sha256"), feature = "sha1"))]
fn size_of_try_finalize_return_type_sha1_only() {
    let oid_size = std::mem::size_of::<ObjectId>();
    let result_size = std::mem::size_of::<Result<ObjectId, gix_hash::hasher::Error>>();
    assert!(
        result_size <= oid_size + 8,
        "The boxed error keeps the Result size close to ObjectId: result={result_size}, oid={oid_size}"
    );
}

#[test]
#[cfg(all(feature = "sha256", feature = "sha1"))]
fn size_of_try_finalize_return_type_sha1_and_sha256() {
    let oid_size = std::mem::size_of::<ObjectId>();
    let result_size = std::mem::size_of::<Result<ObjectId, gix_hash::hasher::Error>>();
    assert!(
        result_size <= oid_size + 8,
        "The boxed error keeps the Result size close to ObjectId: result={result_size}, oid={oid_size}"
    );
}
