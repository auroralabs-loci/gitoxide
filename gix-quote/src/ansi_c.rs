///
pub mod undo {
    /// The error returned by [`ansi_c`][crate::ansi_c::undo()].
    pub type Error = gix_error::Exn<gix_error::Message>;
}

use std::{borrow::Cow, io::Read};

use bstr::{BStr, BString, ByteSlice};
use gix_error::{message, ErrorExt};

/// Unquote the given ansi-c quoted `input` string, returning it and all of the consumed bytes.
///
/// The `input` is returned unaltered if it doesn't start with a `"` character to indicate
/// quotation, otherwise a new unquoted string will always be allocated.
/// The amount of consumed bytes allow to pass strings that start with a quote, and skip all quoted text for additional processing
///
/// See [the tests][tests] for quotation examples.
///
/// [tests]: https://github.com/GitoxideLabs/gitoxide/blob/64872690e60efdd9267d517f4d9971eecd3b875c/gix-quote/tests/quote.rs#L57-L74
pub fn undo(input: &BStr) -> Result<(Cow<'_, BStr>, usize), undo::Error> {
    if !input.starts_with(b"\"") {
        return Ok((input.into(), input.len()));
    }
    if input.len() < 2 {
        return Err(message!("Input must be surrounded by double quotes: {input:?}").raise());
    }
    let original = input.as_bstr();
    let mut input = &input[1..];
    let mut consumed = 1;
    let mut out = BString::default();
    fn consume_one_past(input: &mut &BStr, position: usize) -> Result<u8, undo::Error> {
        use gix_error::{message, ErrorExt};
        *input = input
            .get(position + 1..)
            .ok_or_else(|| message!("Unexpected end of input: {input:?}").raise())?
            .as_bstr();
        let next = *input
            .first()
            .ok_or_else(|| message!("Unexpected end of input: {input:?}").raise())?;
        *input = input.get(1..).unwrap_or_default().as_bstr();
        Ok(next)
    }
    loop {
        match input.find_byteset(b"\"\\") {
            Some(position) => {
                out.extend_from_slice(&input[..position]);
                consumed += position + 1;
                match input[position] {
                    b'"' => break,
                    b'\\' => {
                        let next = consume_one_past(&mut input, position)?;
                        consumed += 1;
                        match next {
                            b'n' => out.push(b'\n'),
                            b'r' => out.push(b'\r'),
                            b't' => out.push(b'\t'),
                            b'a' => out.push(7),
                            b'b' => out.push(8),
                            b'v' => out.push(0xb),
                            b'f' => out.push(0xc),
                            b'"' => out.push(b'"'),
                            b'\\' => out.push(b'\\'),
                            b'0' | b'1' | b'2' | b'3' => {
                                let mut buf = [next; 3];
                                input
                                    .get(..2)
                                    .ok_or_else(|| {
                                        message!(
                                            "Unexpected end of input when fetching two more octal bytes: {input:?}"
                                        )
                                        .raise()
                                    })?
                                    .read_exact(&mut buf[1..])
                                    .expect("impossible to fail as numbers match");
                                let byte = gix_utils::btoi::to_unsigned_with_radix(&buf, 8)
                                    .map_err(|e| message!("{e}: {original:?}").raise())?;
                                out.push(byte);
                                input = &input[2..];
                                consumed += 2;
                            }
                            _ => return Err(message!("Invalid escaped value {next} in input {original:?}").raise()),
                        }
                    }
                    _ => unreachable!("cannot find character that we didn't search for"),
                }
            }
            None => {
                out.extend_from_slice(input);
                consumed += input.len();
                break;
            }
        }
    }
    Ok((out.into(), consumed))
}
