use bstr::{BStr, BString};
use gix_path::RelativePath;

#[cfg(not(windows))]
#[test]
fn absolute_paths_return_err() {
    let path_str: &str = "/refs/heads";
    let path_bstr: &BStr = path_str.into();
    let path_u8a: &[u8; 11] = b"/refs/heads";
    let path_u8: &[u8] = &b"/refs/heads"[..];
    let path_bstring: BString = "/refs/heads".into();

    let err = TryInto::<&RelativePath>::try_into(path_str).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_bstr).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8a).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(&path_bstring).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");
}

#[cfg(windows)]
#[test]
fn absolute_paths_with_backslashes_return_err() {
    let path_str: &str = r"c:\refs\heads";
    let path_bstr: &BStr = path_str.into();
    let path_u8: &[u8] = &b"c:\\refs\\heads"[..];
    let path_bstring: BString = r"c:\refs\heads".into();

    let err = TryInto::<&RelativePath>::try_into(path_str).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_bstr).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(&path_bstring).unwrap_err();
    assert!(err.to_string().contains("not allowed to be absolute"), "{err}");
}

#[test]
fn dots_in_paths_return_err() {
    let path_str: &str = "./heads";
    let path_bstr: &BStr = path_str.into();
    let path_u8: &[u8] = &b"./heads"[..];
    let path_bstring: BString = "./heads".into();

    let err = TryInto::<&RelativePath>::try_into(path_str).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_bstr).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(&path_bstring).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");
}

#[test]
fn dots_in_paths_with_backslashes_return_err() {
    let path_str: &str = r".\heads";
    let path_bstr: &BStr = path_str.into();
    let path_u8: &[u8] = &b".\\heads"[..];
    let path_bstring: BString = r".\heads".into();

    let err = TryInto::<&RelativePath>::try_into(path_str).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_bstr).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(&path_bstring).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");
}

#[test]
fn double_dots_in_paths_return_err() {
    let path_str: &str = "../heads";
    let path_bstr: &BStr = path_str.into();
    let path_u8: &[u8] = &b"../heads"[..];
    let path_bstring: BString = "../heads".into();

    let err = TryInto::<&RelativePath>::try_into(path_str).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_bstr).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(&path_bstring).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");
}

#[test]
fn double_dots_in_paths_with_backslashes_return_err() {
    let path_str: &str = r"..\heads";
    let path_bstr: &BStr = path_str.into();
    let path_u8: &[u8] = &b"..\\heads"[..];
    let path_bstring: BString = r"..\heads".into();

    let err = TryInto::<&RelativePath>::try_into(path_str).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_bstr).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(path_u8).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");

    let err = TryInto::<&RelativePath>::try_into(&path_bstring).unwrap_err();
    assert!(err.to_string().contains("invalid component"), "{err}");
}
