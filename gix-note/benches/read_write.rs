use std::hint::black_box;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use gix_hash::ObjectId;
use gix_object::{
    Kind, Tree, Write,
    tree::{Entry, EntryKind},
};

const NOTES_PER_BYTE: usize = 256;
const NOTE_COUNT: usize = NOTES_PER_BYTE * NOTES_PER_BYTE;

type ObjectDb = gix_odb::memory::Proxy<gix_object::find::Never>;

struct Fixture {
    objects: ObjectDb,
    root_tree_id: ObjectId,
    annotated_object_id: ObjectId,
    note_blob_id: ObjectId,
    replacement_note_blob_id: ObjectId,
}

fn read_write(c: &mut Criterion) {
    for (name, fanout, fixture) in [("fanout-expansion", 1, fixture(1)), ("steady-state", 2, fixture(2))] {
        let mut group = c.benchmark_group(format!("notes/{NOTE_COUNT}-notes/{name}-{fanout}-level-fanout"));
        group.throughput(Throughput::Elements(1));

        group.bench_function("get/new-state", |b| {
            b.iter(|| {
                let mut state = gix_note::State::new(fixture.root_tree_id, &fixture.objects)
                    .expect("read state can be initialized");
                black_box(
                    gix_note::get(&mut state, &fixture.annotated_object_id, &fixture.objects)
                        .expect("the benchmark note can be read"),
                )
            });
        });
        group.bench_function("replace/new-state", |b| {
            b.iter(|| {
                let mut state = gix_note::State::new(fixture.root_tree_id, &fixture.objects)
                    .expect("write state can be initialized");
                black_box(
                    gix_note::replace(
                        &mut state,
                        fixture.annotated_object_id,
                        fixture.replacement_note_blob_id,
                        &fixture.objects,
                    )
                    .expect("the benchmark note can be replaced"),
                )
            });
        });
        let mut read_state =
            gix_note::State::new(fixture.root_tree_id, &fixture.objects).expect("cached read state can be initialized");
        group.bench_function("get/reused-state", |b| {
            b.iter(|| {
                black_box(
                    gix_note::get(&mut read_state, &fixture.annotated_object_id, &fixture.objects)
                        .expect("the cached benchmark note can be read"),
                )
            });
        });

        let mut write_state = gix_note::State::new(fixture.root_tree_id, &fixture.objects)
            .expect("cached write state can be initialized");
        gix_note::replace(
            &mut write_state,
            fixture.annotated_object_id,
            fixture.replacement_note_blob_id,
            &fixture.objects,
        )
        .expect("the write state can be primed outside the measurement");
        let mut use_replacement = true;
        group.bench_function("replace/reused-state", |b| {
            b.iter(|| {
                use_replacement = !use_replacement;
                let note_blob_id = if use_replacement {
                    fixture.replacement_note_blob_id
                } else {
                    fixture.note_blob_id
                };
                black_box(
                    gix_note::replace(
                        &mut write_state,
                        fixture.annotated_object_id,
                        note_blob_id,
                        &fixture.objects,
                    )
                    .expect("the cached benchmark note can be replaced"),
                )
            });
        });
        group.finish();
    }
}

fn fixture(fanout: usize) -> Fixture {
    let objects = ObjectDb::new(gix_object::find::Never, gix_hash::Kind::Sha1);
    let note_blob_id = objects.write_buf(Kind::Blob, b"note").expect("note can be written");
    let replacement_note_blob_id = objects
        .write_buf(Kind::Blob, b"replacement")
        .expect("replacement note can be written");

    let subtree_id = match fanout {
        1 => objects
            .write(&Tree {
                entries: (0..=u8::MAX)
                    .map(|second_byte| {
                        let annotated_object_id = object_id(0, second_byte);
                        Entry {
                            mode: EntryKind::Blob.into(),
                            filename: annotated_object_id.to_hex().to_string()[2..].into(),
                            oid: note_blob_id,
                        }
                    })
                    .collect(),
            })
            .expect("fanout subtree can be written"),
        2 => {
            let annotated_object_id = object_id(0, 0);
            let leaf_tree_id = objects
                .write(&Tree {
                    entries: vec![Entry {
                        mode: EntryKind::Blob.into(),
                        filename: annotated_object_id.to_hex().to_string()[4..].into(),
                        oid: note_blob_id,
                    }],
                })
                .expect("leaf tree can be written");
            objects
                .write(&Tree {
                    entries: (0..=u8::MAX)
                        .map(|second_byte| Entry {
                            mode: EntryKind::Tree.into(),
                            filename: format!("{second_byte:02x}").into(),
                            oid: leaf_tree_id,
                        })
                        .collect(),
                })
                .expect("fanout subtree can be written")
        }
        _ => unreachable!("the benchmark only uses one or two fanout levels"),
    };
    let root_entries = (0..=u8::MAX)
        .map(|first_byte| Entry {
            mode: EntryKind::Tree.into(),
            filename: format!("{first_byte:02x}").into(),
            oid: subtree_id,
        })
        .collect();

    let root_tree_id = objects
        .write(&Tree { entries: root_entries })
        .expect("notes root can be written");
    let annotated_object_id = object_id(0x80, 0x80);
    let mut state = gix_note::State::new(root_tree_id, &objects).expect("read state can be initialized");
    assert_eq!(
        gix_note::get(&mut state, &annotated_object_id, &objects).expect("the fixture can be read"),
        Some(note_blob_id),
        "the fixture contains the benchmark note"
    );

    Fixture {
        objects,
        root_tree_id,
        annotated_object_id,
        note_blob_id,
        replacement_note_blob_id,
    }
}

fn object_id(first_byte: u8, second_byte: u8) -> ObjectId {
    let mut bytes = [0; gix_hash::Kind::Sha1.len_in_bytes()];
    bytes[0] = first_byte;
    bytes[1] = second_byte;
    ObjectId::from_bytes_or_panic(&bytes)
}

criterion_group!(benches, read_write);
criterion_main!(benches);
