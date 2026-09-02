//! Read Git notes from notes trees.
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use gix_error::{CorruptionError, ErrorExt, ResultExt, ValidationError, message};
use gix_hash::{ObjectId, oid};
use gix_object::{
    Find, FindExt, Tree, Write,
    bstr::{BStr, BString, ByteSlice},
    tree::{Editor, EntryKind, EntryMode},
};

/// The type-erased error returned by note operations.
pub type Error = gix_error::Exn;

/// The result of changing one note mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edit {
    /// The root tree containing the changed notes.
    pub tree: ObjectId,
    /// The object ID of the note previously associated with the annotated object.
    ///
    /// This is `Some` when [`replace()`] replaced an existing note or [`remove()`]
    /// removed one. It is `None` when adding a new mapping or when removal
    /// found no matching note. Note IDs are expected to reference blobs, but
    /// their object kind is not verified.
    pub previous: Option<ObjectId>,
}

/// A caller-owned, lazily materialized notes tree for lookups and edits.
///
/// Pass the state to [`get()`], [`replace()`], and [`remove()`] to keep parsed
/// tree entries and opened fanout subtrees in memory. A state starts at one root
/// tree and advances to each tree produced by [`replace()`] or [`remove()`].
///
/// As in Git, the exact fanout can depend on which lazy subtrees were materialized.
/// Discard the state if an operation fails.
#[doc(alias = "notes_tree")]
pub struct State {
    root_tree_id: ObjectId,
    root: InternalNode,
    non_notes: Vec<TreeEntry>,
}

impl State {
    /// Initialize state from `root_tree_id`, loading only its root tree.
    pub fn new(root_tree_id: ObjectId, objects: &impl Find) -> Result<Self, Error> {
        let mut root = InternalNode::default();
        let mut non_notes = Vec::new();
        load_subtree(
            Subtree {
                prefix: ObjectId::null(root_tree_id.kind()),
                prefix_len: 0,
                path: Vec::new(),
                tree_id: root_tree_id,
            },
            &mut root,
            0,
            objects,
            &mut non_notes,
        )?;
        Ok(State {
            root_tree_id,
            root,
            non_notes,
        })
    }

    /// Return the root tree represented by this state.
    pub fn root_tree_id(&self) -> ObjectId {
        self.root_tree_id
    }

    fn edit(
        &mut self,
        annotated_object_id: ObjectId,
        note_blob_id: Option<ObjectId>,
        objects: &(impl Find + Write),
    ) -> Result<Edit, Error> {
        let previous_note_blob_id = self
            .root
            .remove(&annotated_object_id, 0, objects, &mut self.non_notes)?;
        if note_blob_id.is_none() && previous_note_blob_id.is_none() {
            return Ok(Edit {
                tree: self.root_tree_id,
                previous: previous_note_blob_id,
            });
        }
        if let Some(note_blob_id) = note_blob_id {
            self.root.insert(
                Node::Note(Note {
                    annotated_object_id,
                    note_blob_id,
                }),
                0,
                objects,
                &mut self.non_notes,
            )?;
        }
        self.root_tree_id = write(&mut self.root, &mut self.non_notes, self.root_tree_id.kind(), objects)?;
        Ok(Edit {
            tree: self.root_tree_id,
            previous: previous_note_blob_id,
        })
    }
}

/// Return the note associated with `annotated_object_id` in `state`.
///
/// Git notes are expected to reference blobs. This function verifies that the
/// notes-tree entry has blob mode, but does not load the referenced object to
/// verify its actual kind.
///
/// Fanout subtrees on the lookup path are materialized once and retained for
/// subsequent operations. Entries that do not conform to Git's notes layout
/// are ignored.
pub fn get(state: &mut State, annotated_object_id: &oid, objects: &impl Find) -> Result<Option<ObjectId>, Error> {
    validate_annotated_object_kind(state.root_tree_id.kind(), annotated_object_id)?;
    state.root.get(annotated_object_id, 0, objects, &mut state.non_notes)
}

/// Replace the note for `object`, or add it if absent, returning the new root
/// tree and any previous note.
///
/// The notes tree is rewritten with the same progressive fanout heuristic as
/// Git while retaining entries that are not notes. Untouched fanout subtrees
/// are kept by object ID and loaded only when the edit or a fanout change needs
/// their contents. `note` is expected to reference a blob, but its actual object
/// kind is not verified; the mapping is always written as a blob-mode tree
/// entry.
pub fn replace(
    state: &mut State,
    annotated_object_id: ObjectId,
    note_blob_id: ObjectId,
    objects: &(impl Find + Write),
) -> Result<Edit, Error> {
    validate_replace_kinds(
        state.root_tree_id.kind(),
        annotated_object_id.kind(),
        note_blob_id.kind(),
    )?;
    state.edit(annotated_object_id, Some(note_blob_id), objects)
}

/// Remove the note for `object`, returning the new root tree and removed note.
///
/// If there is no such note, the root is returned unchanged.
pub fn remove(state: &mut State, annotated_object_id: ObjectId, objects: &(impl Find + Write)) -> Result<Edit, Error> {
    validate_annotated_object_kind(state.root_tree_id.kind(), &annotated_object_id)?;
    state.edit(annotated_object_id, None, objects)
}

fn validate_replace_kinds(
    root: gix_hash::Kind,
    annotated_object: gix_hash::Kind,
    note_blob: gix_hash::Kind,
) -> Result<(), Error> {
    if annotated_object != root || note_blob != root {
        return Err(
            ValidationError::from("Notes, annotated objects, and their root tree must use the same hash kind")
                .raise_erased(),
        );
    }
    Ok(())
}

fn validate_annotated_object_kind(root: gix_hash::Kind, annotated_object_id: &oid) -> Result<(), Error> {
    if annotated_object_id.kind() != root {
        return Err(
            ValidationError::from("The annotated object and notes root tree must use the same hash kind")
                .raise_erased(),
        );
    }
    Ok(())
}

struct TreeEntry {
    path: Vec<BString>,
    mode: EntryMode,
    object_id: ObjectId,
}

#[derive(Default)]
struct InternalNode {
    children: [Option<Box<Node>>; 16],
}

enum Node {
    Internal(InternalNode),
    Note(Note),
    Subtree(Subtree),
}

#[derive(Clone, Copy)]
struct Note {
    annotated_object_id: ObjectId,
    note_blob_id: ObjectId,
}

// An unopened on-disk fanout directory. Keeping it opaque is what makes edits path-local.
#[derive(Clone)]
struct Subtree {
    prefix: ObjectId,
    prefix_len: usize,
    path: Vec<BString>,
    tree_id: ObjectId,
}

impl Node {
    fn key(&self) -> &oid {
        match self {
            Node::Note(note) => &note.annotated_object_id,
            Node::Subtree(subtree) => &subtree.prefix,
            Node::Internal(_) => unreachable!("internal nodes have no object ID key"),
        }
    }
}

impl Subtree {
    fn contains(&self, id: &oid) -> bool {
        self.prefix.as_bytes()[..self.prefix_len] == id.as_bytes()[..self.prefix_len]
    }
}

impl InternalNode {
    fn get(
        &mut self,
        annotated_object_id: &oid,
        nibble: usize,
        objects: &impl Find,
        non_notes: &mut Vec<TreeEntry>,
    ) -> Result<Option<ObjectId>, Error> {
        if self.load_matching_subtree(annotated_object_id, nibble, objects, non_notes)? {
            return self.get(annotated_object_id, nibble, objects, non_notes);
        }

        let index = nibble_at(annotated_object_id, nibble);
        let should_load = self.children[index]
            .as_deref()
            .is_some_and(|node| matches!(node, Node::Subtree(subtree) if subtree.contains(annotated_object_id)));
        if should_load {
            let Node::Subtree(subtree) = *self.children[index].take().expect("the matching subtree is present") else {
                unreachable!("the matching node was checked to be a subtree")
            };
            load_subtree(subtree, self, nibble, objects, non_notes)?;
            return self.get(annotated_object_id, nibble, objects, non_notes);
        }

        match self.children[index].as_deref_mut() {
            Some(Node::Internal(child)) => child.get(annotated_object_id, nibble + 1, objects, non_notes),
            Some(Node::Note(note)) if note.annotated_object_id == annotated_object_id => Ok(Some(note.note_blob_id)),
            _ => Ok(None),
        }
    }

    fn insert(
        &mut self,
        entry: Node,
        nibble: usize,
        objects: &impl Find,
        non_notes: &mut Vec<TreeEntry>,
    ) -> Result<(), Error> {
        if self.load_matching_subtree(entry.key(), nibble, objects, non_notes)? {
            return self.insert(entry, nibble, objects, non_notes);
        }

        let index = nibble_at(entry.key(), nibble);
        let Some(existing) = self.children[index].take() else {
            self.children[index] = Some(Box::new(entry));
            return Ok(());
        };

        match *existing {
            Node::Internal(mut child) => {
                let result = child.insert(entry, nibble + 1, objects, non_notes);
                self.children[index] = Some(Box::new(Node::Internal(child)));
                result
            }
            Node::Note(note) => {
                if matches!(&entry, Node::Note(incoming) if incoming.annotated_object_id == note.annotated_object_id) {
                    return Err(CorruptionError::from(format!(
                        "Multiple notes map to object {}",
                        note.annotated_object_id
                    ))
                    .raise_erased());
                }
                if let Node::Subtree(subtree) = &entry
                    && subtree.contains(&note.annotated_object_id)
                {
                    self.children[index] = Some(Box::new(Node::Note(note)));
                    return load_subtree(subtree.clone(), self, nibble, objects, non_notes);
                }
                self.insert_collision(Node::Note(note), entry, index, nibble, objects, non_notes)
            }
            Node::Subtree(subtree) => {
                if subtree.contains(entry.key()) {
                    load_subtree(subtree, self, nibble, objects, non_notes)?;
                    self.insert(entry, nibble, objects, non_notes)
                } else {
                    self.insert_collision(Node::Subtree(subtree), entry, index, nibble, objects, non_notes)
                }
            }
        }
    }

    fn insert_collision(
        &mut self,
        existing: Node,
        entry: Node,
        index: usize,
        nibble: usize,
        objects: &impl Find,
        non_notes: &mut Vec<TreeEntry>,
    ) -> Result<(), Error> {
        let mut child = InternalNode::default();
        child.insert(existing, nibble + 1, objects, non_notes)?;
        child.insert(entry, nibble + 1, objects, non_notes)?;
        self.children[index] = Some(Box::new(Node::Internal(child)));
        Ok(())
    }

    fn remove(
        &mut self,
        annotated_object_id: &oid,
        nibble: usize,
        objects: &impl Find,
        non_notes: &mut Vec<TreeEntry>,
    ) -> Result<Option<ObjectId>, Error> {
        if self.load_matching_subtree(annotated_object_id, nibble, objects, non_notes)? {
            return self.remove(annotated_object_id, nibble, objects, non_notes);
        }

        let index = nibble_at(annotated_object_id, nibble);
        let Some(existing) = self.children[index].take() else {
            return Ok(None);
        };
        match *existing {
            Node::Internal(mut child) => {
                let previous = child.remove(annotated_object_id, nibble + 1, objects, non_notes)?;
                self.children[index] = if previous.is_some() {
                    child.after_removal()
                } else {
                    Some(Box::new(Node::Internal(child)))
                };
                Ok(previous)
            }
            Node::Note(note) if note.annotated_object_id == annotated_object_id => Ok(Some(note.note_blob_id)),
            Node::Note(note) => {
                self.children[index] = Some(Box::new(Node::Note(note)));
                Ok(None)
            }
            Node::Subtree(subtree) if subtree.contains(annotated_object_id) => {
                load_subtree(subtree, self, nibble, objects, non_notes)?;
                self.remove(annotated_object_id, nibble, objects, non_notes)
            }
            Node::Subtree(subtree) => {
                self.children[index] = Some(Box::new(Node::Subtree(subtree)));
                Ok(None)
            }
        }
    }

    fn load_matching_subtree(
        &mut self,
        key: &oid,
        nibble: usize,
        objects: &impl Find,
        non_notes: &mut Vec<TreeEntry>,
    ) -> Result<bool, Error> {
        let is_match = self.children[0]
            .as_deref()
            .is_some_and(|node| matches!(node, Node::Subtree(subtree) if subtree.contains(key)));
        if !is_match {
            return Ok(false);
        }
        let Node::Subtree(subtree) = *self.children[0].take().expect("the matching subtree is present") else {
            unreachable!("the matching node was checked to be a subtree")
        };
        load_subtree(subtree, self, nibble, objects, non_notes)?;
        Ok(true)
    }

    fn after_removal(mut self) -> Option<Box<Node>> {
        let mut occupied = self.children.iter().enumerate().filter(|(_, child)| child.is_some());
        let (only_index, _) = occupied.next()?;
        if occupied.next().is_none()
            && self.children[only_index]
                .as_deref()
                .is_some_and(|node| matches!(node, Node::Note(_)))
        {
            return self.children[only_index].take();
        }
        Some(Box::new(Node::Internal(self)))
    }
}

fn load_subtree(
    subtree: Subtree,
    node: &mut InternalNode,
    nibble: usize,
    objects: &impl Find,
    non_notes: &mut Vec<TreeEntry>,
) -> Result<(), Error> {
    let mut buf = Vec::new();
    let tree = objects
        .find_tree(&subtree.tree_id, &mut buf)
        .or_raise_erased(|| message!("Could not load notes tree {}", subtree.tree_id))?;
    let hex_len = subtree.tree_id.kind().len_in_hex();
    let prefix_hex_len = subtree.prefix_len * 2;
    let mut prefix_hex = gix_hash::Kind::hex_buf();
    prefix_hex.fill(b'0');
    let _ = subtree.prefix.hex_to_buf(&mut prefix_hex);
    for entry in tree.entries {
        if entry.mode.is_blob() && entry.filename.len() + prefix_hex_len == hex_len {
            let mut hex = prefix_hex;
            hex[prefix_hex_len..hex_len].copy_from_slice(entry.filename);
            if let Ok(annotated_object_id) = ObjectId::from_hex(&hex[..hex_len]) {
                node.insert(
                    Node::Note(Note {
                        annotated_object_id,
                        note_blob_id: entry.oid.to_owned(),
                    }),
                    nibble,
                    objects,
                    non_notes,
                )?;
                continue;
            }
        }
        if entry.mode.is_tree()
            && entry.filename.len() == 2
            && prefix_hex_len + 2 < hex_len
            && entry.filename.iter().all(u8::is_ascii_hexdigit)
        {
            let mut path = subtree.path.clone();
            path.push(entry.filename.to_owned());
            let mut hex = prefix_hex;
            hex[prefix_hex_len..prefix_hex_len + 2].copy_from_slice(entry.filename);
            let prefix = ObjectId::from_hex(&hex[..hex_len]).expect("validated hex produces an object ID");
            node.insert(
                Node::Subtree(Subtree {
                    prefix,
                    prefix_len: subtree.prefix_len + 1,
                    path,
                    tree_id: entry.oid.to_owned(),
                }),
                nibble,
                objects,
                non_notes,
            )?;
        } else {
            let mut path = subtree.path.clone();
            path.push(entry.filename.to_owned());
            non_notes.push(TreeEntry {
                path,
                mode: entry.mode,
                object_id: entry.oid.to_owned(),
            });
        }
    }
    Ok(())
}

fn write(
    root: &mut InternalNode,
    non_notes: &mut Vec<TreeEntry>,
    hash: gix_hash::Kind,
    objects: &(impl Find + Write),
) -> Result<ObjectId, Error> {
    let mut notes = Vec::new();
    collect_for_write(root, 0, 0, objects, non_notes, &mut notes)?;
    let mut editor = Editor::new(Tree { entries: Vec::new() }, objects, hash);
    for entry in non_notes.iter() {
        editor
            .upsert(entry.path.iter(), entry.mode.kind(), entry.object_id)
            .or_raise_erased(|| message("Could not restore a non-note tree entry"))?;
    }
    for entry in notes {
        editor
            .upsert(entry.path.iter(), entry.mode.kind(), entry.object_id)
            .or_raise_erased(|| message("Could not add a note tree entry"))?;
    }
    editor
        .write(|tree| objects.write(tree).map_err(gix_error::Error::from_boxed))
        .or_raise_erased(|| message("Could not write the notes tree"))
}

fn collect_for_write(
    node: &mut InternalNode,
    nibble: usize,
    fanout: usize,
    objects: &impl Find,
    non_notes: &mut Vec<TreeEntry>,
    notes: &mut Vec<TreeEntry>,
) -> Result<(), Error> {
    let fanout = if nibble.is_multiple_of(2)
        && nibble <= 2 * fanout
        && node
            .children
            .iter()
            .all(|child| matches!(child.as_deref(), Some(Node::Internal(_) | Node::Subtree(_))))
    {
        fanout + 1
    } else {
        fanout
    };

    for index in 0..node.children.len() {
        while let Some(entry) = node.children[index].take() {
            match *entry {
                Node::Internal(mut child) => {
                    collect_for_write(&mut child, nibble + 1, fanout, objects, non_notes, notes)?;
                    node.children[index] = Some(Box::new(Node::Internal(child)));
                    break;
                }
                Node::Note(note) => {
                    notes.push(TreeEntry {
                        path: note_path_components(&note.annotated_object_id, fanout),
                        mode: EntryKind::Blob.into(),
                        object_id: note.note_blob_id,
                    });
                    node.children[index] = Some(Box::new(Node::Note(note)));
                    break;
                }
                Node::Subtree(subtree) if nibble < 2 * fanout => {
                    notes.push(TreeEntry {
                        path: subtree.path.clone(),
                        mode: EntryKind::Tree.into(),
                        object_id: subtree.tree_id,
                    });
                    node.children[index] = Some(Box::new(Node::Subtree(subtree)));
                    break;
                }
                Node::Subtree(subtree) => {
                    load_subtree(subtree, node, nibble, objects, non_notes)?;
                }
            }
        }
    }
    Ok(())
}

fn nibble_at(id: &oid, nibble: usize) -> usize {
    let byte = id.as_bytes()[nibble / 2];
    usize::from(if nibble.is_multiple_of(2) {
        byte >> 4
    } else {
        byte & 0x0f
    })
}

fn note_path_components(id: &oid, fanout: usize) -> Vec<BString> {
    // A notes path contains the full hexadecimal ID plus one slash for each byte consumed as a fanout directory.
    // At least one byte remains for the leaf name, so allowing one slash per hash byte is a simple one-byte overestimate.
    const NOTE_PATH_BUFFER_SIZE: usize =
        gix_hash::Kind::longest().len_in_hex() + gix_hash::Kind::longest().len_in_bytes();

    let mut path_buf = [0u8; NOTE_PATH_BUFFER_SIZE];
    let path = note_path(id, fanout, &mut path_buf);
    path.split_str("/").map(BString::from).collect()
}

/// Write the notes-tree path for `id` with `fanout` leading bytes represented as directory components.
///
/// For an ID beginning with `01234567…`, fanout `0` produces `01234567…`, fanout `1` produces `01/234567…`, and
/// fanout `2` produces `01/23/4567…`. The returned path borrows the initialized portion of `out`.
fn note_path<'a>(id: &oid, fanout: usize, out: &'a mut [u8]) -> &'a BStr {
    let mut pos = 0;
    for offset in 0..fanout {
        let component = id.to_prefix(offset..offset + 1);
        pos += component.hex_to_buf(&mut out[pos..]).len();
        out[pos] = b'/';
        pos += 1;
    }
    let remainder = id.to_prefix(fanout..id.as_bytes().len());
    pos += remainder.hex_to_buf(&mut out[pos..]).len();
    BStr::new(&out[..pos])
}
