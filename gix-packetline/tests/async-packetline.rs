pub type Result = std::result::Result<(), Box<dyn std::error::Error>>;

pub fn assert_err_display<T: std::fmt::Debug, E: std::error::Error>(
    res: std::result::Result<T, E>,
    expected: impl AsRef<str>,
) {
    match res {
        Ok(v) => panic!("Expected error '{}', got value {:?}", expected.as_ref(), v),
        Err(err) => {
            let msg = err.to_string();
            assert!(
                msg.starts_with(expected.as_ref()),
                "Expected error starting with '{}', got '{msg}'",
                expected.as_ref()
            );
        }
    }
}

#[cfg(all(feature = "async-io", not(feature = "blocking-io")))]
mod decode;
#[cfg(all(feature = "async-io", not(feature = "blocking-io")))]
mod encode;
#[cfg(all(feature = "async-io", not(feature = "blocking-io")))]
mod read;
#[cfg(all(feature = "async-io", not(feature = "blocking-io")))]
mod write;
