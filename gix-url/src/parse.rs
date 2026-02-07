use bstr::{BStr, BString, ByteSlice};
use gix_error::{message, ErrorExt, ResultExt};

use crate::Scheme;

/// The error returned by [parse()](crate::parse()).
#[allow(missing_docs)]
pub type Error = gix_error::Exn<gix_error::Message>;

///
#[derive(Debug, Clone, Copy)]
pub enum UrlKind {
    ///
    Url,
    ///
    Scp,
    ///
    Local,
}

impl UrlKind {
    fn as_str(&self) -> &'static str {
        match self {
            UrlKind::Url => "URL",
            UrlKind::Scp => "SCP-like target",
            UrlKind::Local => "local path",
        }
    }
}

pub(crate) enum InputScheme {
    Url { protocol_end: usize },
    Scp { colon: usize },
    Local,
}

pub(crate) fn find_scheme(input: &BStr) -> InputScheme {
    // TODO: url's may only contain `:/`, we should additionally check if the characters used for
    //       protocol are all valid
    if let Some(protocol_end) = input.find("://") {
        return InputScheme::Url { protocol_end };
    }

    // Find colon, but skip over IPv6 brackets if present
    let colon = if input.starts_with(b"[") {
        // IPv6 address, find the closing bracket first
        if let Some(bracket_end) = input.find_byte(b']') {
            // Look for colon after the bracket
            input[bracket_end + 1..]
                .find_byte(b':')
                .map(|pos| bracket_end + 1 + pos)
        } else {
            // No closing bracket, treat as regular search
            input.find_byte(b':')
        }
    } else {
        input.find_byte(b':')
    };

    if let Some(colon) = colon {
        // allow user to select files containing a `:` by passing them as absolute or relative path
        // this is behavior explicitly mentioned by the scp and git manuals
        let explicitly_local = &input[..colon].contains(&b'/');
        let dos_driver_letter = cfg!(windows) && input[..colon].len() == 1;

        if !explicitly_local && !dos_driver_letter {
            return InputScheme::Scp { colon };
        }
    }

    InputScheme::Local
}

pub(crate) fn url(input: &BStr, protocol_end: usize) -> Result<crate::Url, Error> {
    const MAX_LEN: usize = 1024;
    let bytes_to_path = input[protocol_end + "://".len()..]
        .iter()
        .filter(|b| !b.is_ascii_whitespace())
        .skip_while(|b| **b == b'/' || **b == b'\\')
        .position(|b| *b == b'/')
        .unwrap_or(input.len() - protocol_end);
    if bytes_to_path > MAX_LEN || protocol_end > MAX_LEN {
        let truncated_url: bstr::BString = input[..(protocol_end + "://".len() + MAX_LEN).min(input.len())].into();
        let len = input.len();
        return Err(message!("The host portion of the following URL is too long ({} bytes, {len} bytes total): {truncated_url:?}", truncated_url.len()).raise());
    }
    let (input, url) = input_to_utf8_and_url(input, UrlKind::Url)?;
    let scheme = Scheme::from(url.scheme.as_str());

    if matches!(scheme, Scheme::Git | Scheme::Ssh) && url.path.is_empty() {
        return Err(message!("{} \"{input}\" does not specify a path to a repository", UrlKind::Url.as_str()).raise());
    }

    // Normalize empty path to "/" for http/https URLs only
    let path: BString = if url.path.is_empty() && matches!(scheme, Scheme::Http | Scheme::Https) {
        "/".into()
    } else if matches!(scheme, Scheme::Ssh | Scheme::Git) && url.path.starts_with("/~") {
        // For SSH and Git protocols, strip leading '/' from paths starting with '~'
        // e.g., "ssh://host/~repo" -> path is "~repo", not "/~repo"
        url.path[1..].into()
    } else {
        url.path.into()
    };

    let user = if url.username.is_empty() && url.password.is_none() {
        None
    } else {
        Some(url.username)
    };
    let password = url.password;
    let port = url.port;

    // For SSH URLs, strip brackets from IPv6 addresses
    let host = if scheme == Scheme::Ssh {
        url.host.map(|mut h| {
            // Bracketed IPv6 forms
            if let Some(h2) = h.strip_prefix('[') {
                if let Some(inner) = h2.strip_suffix("]:") {
                    // "[::1]:" → "::1"
                    h = inner.to_string();
                } else if let Some(inner) = h2.strip_suffix(']') {
                    // "[::1]" → "::1"
                    h = inner.to_string();
                }
            } else {
                // Non-bracketed host: strip a single trailing colon
                let colon_count = h.chars().filter(|&c| c == ':').take(2).count();
                if colon_count == 1 {
                    if let Some(inner) = h.strip_suffix(':') {
                        h = inner.to_string();
                    }
                }
            }
            h
        })
    } else {
        url.host
    };
    Ok(crate::Url {
        serialize_alternative_form: false,
        scheme,
        user,
        password,
        host,
        port,
        path,
    })
}

pub(crate) fn scp(input: &BStr, colon: usize) -> Result<crate::Url, Error> {
    let input = input_to_utf8(input, UrlKind::Scp)?;

    // TODO: this incorrectly splits at IPv6 addresses, check for `[]` before splitting
    let (host, path) = input.split_at(colon);
    debug_assert_eq!(path.get(..1), Some(":"), "{path} should start with :");
    let path = &path[1..];

    if path.is_empty() {
        return Err(message!("{} \"{}\" does not specify a path to a repository", UrlKind::Scp.as_str(), input).raise());
    }

    // The path returned by the parsed url often has the wrong number of leading `/` characters but
    // should never differ in any other way (ssh URLs should not contain a query or fragment part).
    // To avoid the various off-by-one errors caused by the `/` characters, we keep using the path
    // determined above and can therefore skip parsing it here as well.
    let url_string = format!("ssh://{host}");
    let url = crate::simple_url::ParsedUrl::parse(&url_string)
        .or_raise(|| message!("{} {:?} can not be parsed as valid URL", UrlKind::Scp.as_str(), input))?;

    // For SCP-like SSH URLs, strip leading '/' from paths starting with '/~'
    // e.g., "user@host:/~repo" -> path is "~repo", not "/~repo"
    let path = if path.starts_with("/~") { &path[1..] } else { path };

    let user = if url.username.is_empty() && url.password.is_none() {
        None
    } else {
        Some(url.username)
    };
    let password = url.password;
    let port = url.port;

    // For SCP-like SSH URLs, strip brackets from IPv6 addresses
    let host = url.host.map(|h| {
        if let Some(h) = h.strip_prefix("[").and_then(|h| h.strip_suffix("]")) {
            h.to_string()
        } else {
            h
        }
    });

    Ok(crate::Url {
        serialize_alternative_form: true,
        scheme: Scheme::from(url.scheme.as_str()),
        user,
        password,
        host,
        port,
        path: path.into(),
    })
}

pub(crate) fn file_url(input: &BStr, protocol_colon: usize) -> Result<crate::Url, Error> {
    let input = input_to_utf8(input, UrlKind::Url)?;
    let input_after_protocol = &input[protocol_colon + "://".len()..];

    let Some(first_slash) = input_after_protocol
        .find('/')
        .or_else(|| cfg!(windows).then(|| input_after_protocol.find('\\')).flatten())
    else {
        return Err(message!("{} \"{}\" does not specify a path to a repository", UrlKind::Url.as_str(), input).raise());
    };

    // We cannot use the url crate to parse host and path because it special cases Windows
    // driver letters. With the url crate an input of `file://x:/path/to/git` is parsed as empty
    // host and with `x:/path/to/git` as path. This behavior is wrong for Git which only follows
    // that rule on Windows and parses `x:` as host on Unix platforms. Additionally, the url crate
    // does not account for Windows special UNC path support.

    // TODO: implement UNC path special case
    let windows_special_path = if cfg!(windows) {
        // Inputs created via url::Url::from_file_path contain an additional `/` between the
        // protocol and the absolute path. Make sure we ignore that first slash character to avoid
        // producing invalid paths.
        let input_after_protocol = if first_slash == 0 {
            &input_after_protocol[1..]
        } else {
            input_after_protocol
        };
        // parse `file://x:/path/to/git` as explained above
        if input_after_protocol.chars().nth(1) == Some(':') {
            Some(input_after_protocol)
        } else {
            None
        }
    } else {
        None
    };

    let host = if windows_special_path.is_some() || first_slash == 0 {
        // `file:///path/to/git` or a windows special case was triggered
        None
    } else {
        // `file://host/path/to/git`
        Some(&input_after_protocol[..first_slash])
    };

    // default behavior on Unix platforms and if no Windows special case was triggered
    let path = windows_special_path.unwrap_or(&input_after_protocol[first_slash..]);

    Ok(crate::Url {
        serialize_alternative_form: false,
        host: host.map(Into::into),
        ..local(path.into())?
    })
}

pub(crate) fn local(input: &BStr) -> Result<crate::Url, Error> {
    if input.is_empty() {
        return Err(message!("{} \"{}\" does not specify a path to a repository", UrlKind::Local.as_str(), input).raise());
    }

    Ok(crate::Url {
        serialize_alternative_form: true,
        scheme: Scheme::File,
        password: None,
        user: None,
        host: None,
        port: None,
        path: input.to_owned(),
    })
}

fn input_to_utf8(input: &BStr, kind: UrlKind) -> Result<&str, Error> {
    std::str::from_utf8(input)
        .or_raise(|| message!("{} \"{}\" is not valid UTF-8", kind.as_str(), input))
}

fn input_to_utf8_and_url(input: &BStr, kind: UrlKind) -> Result<(&str, crate::simple_url::ParsedUrl), Error> {
    let input = input_to_utf8(input, kind)?;
    crate::simple_url::ParsedUrl::parse(input)
        .map(|url| (input, url))
        .map_err(|source| {
            // If the parser rejected it as "relative URL without a base", map to RelativeUrl
            // to match the expected error type for malformed URLs like "invalid:://"
            if source.to_string().contains("relative URL without a base") {
                message!("URL {:?} is relative which is not allowed in this context", input).raise()
            } else {
                source.raise(message!("{} {:?} can not be parsed as valid URL", kind.as_str(), input))
            }
        })
}
