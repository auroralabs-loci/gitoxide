use crate::parse::parse;

#[test]
fn relative_path_due_to_double_colon() {
    let err = parse("invalid:://host.xz/path/to/repo.git/").unwrap_err();
    assert!(
        err.to_string()
            .contains("is relative which is not allowed in this context"),
        "unexpected error: {err}"
    );
}

#[test]
fn ssh_missing_path() {
    let err = parse("ssh://host.xz").unwrap_err();
    assert!(
        err.to_string().contains("does not specify a path to a repository"),
        "unexpected error: {err}"
    );
}

#[test]
fn git_missing_path() {
    let err = parse("git://host.xz").unwrap_err();
    assert!(
        err.to_string().contains("does not specify a path to a repository"),
        "unexpected error: {err}"
    );
}

#[test]
fn file_missing_path() {
    let err = parse("file://").unwrap_err();
    assert!(
        err.to_string().contains("does not specify a path to a repository"),
        "unexpected error: {err}"
    );
}

#[test]
fn empty_input() {
    let err = parse("").unwrap_err();
    assert!(
        err.to_string().contains("does not specify a path to a repository"),
        "unexpected error: {err}"
    );
}

#[test]
fn file_missing_host_path_separator() {
    for input in ["file://..", "file://.", "file://a"] {
        let err = parse(input).unwrap_err();
        assert!(
            err.to_string().contains("does not specify a path to a repository"),
            "unexpected error for {input:?}: {err}"
        );
    }
}

#[test]
fn missing_port_despite_indication() {
    let err = parse("ssh://host.xz:").unwrap_err();
    assert!(
        err.to_string().contains("does not specify a path to a repository"),
        "unexpected error: {err}"
    );
}

#[test]
fn port_zero_is_invalid() {
    let err = parse("ssh://host.xz:0/path").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn port_too_large() {
    for input in ["ssh://host.xz:65536/path", "ssh://host.xz:99999/path"] {
        let err = parse(input).unwrap_err();
        assert!(
            err.to_string().contains("can not be parsed as valid URL"),
            "unexpected error for {input:?}: {err}"
        );
    }
}

#[test]
fn invalid_port_format() {
    let url = parse("ssh://host.xz:abc/path").expect("non-numeric port is treated as part of host");
    assert_eq!(
        url.host(),
        Some("host.xz:abc"),
        "port parse failure makes it part of hostname"
    );
    assert_eq!(url.port, None);
}

#[test]
fn host_with_space() {
    for input in [
        "http://has a space",
        "http://has a space/path",
        "https://example.com with space/path",
    ] {
        let err = parse(input).unwrap_err();
        assert!(
            err.to_string().contains("can not be parsed as valid URL"),
            "unexpected error for {input:?}: {err}"
        );
    }
}

#[test]
fn url_with_space_in_path() {
    // Spaces in path should be rejected for http URLs per RFC 3986
    let err = parse("http://example.com/ path").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn url_with_space_in_username() {
    // Spaces in username should be rejected for http URLs per RFC 3986
    let err = parse("http://user name@example.com/path").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn url_with_space_in_password() {
    // Spaces in password should be rejected for http URLs per RFC 3986
    let err = parse("http://user:pass word@example.com/path").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn url_with_tab_in_path() {
    // Tabs in path should be rejected for http URLs per RFC 3986
    let err = parse("http://example.com/\tpath").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn url_with_newline_in_path() {
    // Newlines in path should be rejected for http URLs per RFC 3986
    let err = parse("http://example.com/\npath").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn url_with_tab_in_username() {
    // Tabs in username should be rejected for http URLs per RFC 3986
    let err = parse("http://user\tname@example.com/path").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}

#[test]
fn url_with_tab_in_password() {
    // Tabs in password should be rejected for http URLs per RFC 3986
    let err = parse("http://user:pass\tword@example.com/path").unwrap_err();
    assert!(
        err.to_string().contains("can not be parsed as valid URL"),
        "unexpected error: {err}"
    );
}
