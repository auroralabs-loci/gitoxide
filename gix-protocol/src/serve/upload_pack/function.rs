use std::io::{self, Read, Write};

use gix_hash::ObjectId;

use crate::serve::upload_pack::{parse_haves, parse_wants, write_ack, write_nak, AckStatus, Error};
use crate::serve::{write_v1, RefAdvertisement};
use crate::transport::server::blocking_io::connection::Connection;

/// Serve a V1 upload-pack session.
pub fn serve_upload_pack_v1<R: Read, W: Write>(
    connection: &mut Connection<R, W>,
    refs: &[RefAdvertisement<'_>],
    has_object: impl Fn(&gix_hash::oid) -> bool,
    generate_pack: impl FnOnce(&[ObjectId], &[ObjectId], &mut dyn Write) -> io::Result<()>,
    capabilities: &[&str],
) -> Result<(), Error> {
    write_v1(&mut connection.writer, refs, capabilities)?;

    let wants = parse_wants(&mut connection.line_provider)?;
    if wants.wants.is_empty() {
        return Ok(());
    };
    connection.line_provider.reset();

    let mut common = Vec::new();
    loop {
        let haves = parse_haves(&mut connection.line_provider)?;
        let mut found_common = false;
        for oid in &haves.haves {
            if has_object(oid) {
                write_ack(&mut connection.writer, oid, AckStatus::Common)?;
                common.push(*oid);
                found_common = true;
            }
        }

        if !found_common {
            write_nak(&mut connection.writer)?;
        }

        if haves.done {
            break;
        }
        connection.line_provider.reset();
    }

    if let Some(last) = common.last() {
        write_ack(&mut connection.writer, last, AckStatus::Final)?;
    } else {
        write_nak(&mut connection.writer)?;
    };

    let want_ids: Vec<ObjectId> = wants.wants.iter().map(|w| w.id).collect();
    generate_pack(&want_ids, &common, &mut connection.writer)?;

    Ok(())
}
