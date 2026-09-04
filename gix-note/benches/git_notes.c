/*
 * Disposable comparison for:
 * notes/65536-notes/fanout-expansion-1-level-fanout/replace/reused-state
 *
 * Build from a current Git source tree after `make libgit.a`:
 *
 *   cc -std=gnu23 -O3 -fno-common -I. -DNO_OPENSSL -DNO_GETTEXT \
 *     /path/to/gitoxide/gix-note/benches/git_notes.c varint.c libgit.a \
 *     -Wl,-dead_strip \
 *     -framework CoreServices -L/opt/homebrew/opt/gettext/lib \
 *     -lz -liconv -lpthread -o /tmp/git-notes-bench
 *
 * This includes Git's notes.c so the benchmark can construct its private,
 * already-materialized notes-tree state. Only odb_write_object() is replaced.
 */
#define USE_THE_REPOSITORY_VARIABLE
#define DISABLE_SIGN_COMPARE_WARNINGS

#include "git-compat-util.h"
#include "notes.h"
#include "object-file.h"
#include "odb/source.h"
#include "repository.h"

static int bench_write_object(struct object_database *odb, const void *buf,
			      unsigned long len, enum object_type type,
			      struct object_id *oid);

#define odb_write_object bench_write_object
#include "notes.c"
#undef odb_write_object

static uint64_t object_writes;

static int bench_write_object(struct object_database *odb, const void *buf,
			      unsigned long len, enum object_type type,
			      struct object_id *oid)
{
	object_writes++;
	return odb_pretend_object(odb, (void *)buf, len, type, oid);
}

static struct leaf_node *leaf(const struct object_id *key,
			      const struct object_id *value)
{
	struct leaf_node *out;

	CALLOC_ARRAY(out, 1);
	oidcpy(&out->key_oid, key);
	oidcpy(&out->val_oid, value);
	return out;
}

static struct object_id annotated_oid(unsigned first, unsigned second)
{
	struct object_id oid;

	oidclr(&oid, the_repository->hash_algo);
	oid.hash[0] = first;
	oid.hash[1] = second;
	return oid;
}

static struct object_id subtree_prefix(unsigned first)
{
	struct object_id oid;

	oidclr(&oid, the_repository->hash_algo);
	oid.hash[0] = first;
	oid.hash[KEY_INDEX] = 1;
	return oid;
}

static void insert_fixture(struct notes_tree *notes,
			   const struct object_id *subtree_oid,
			   const struct object_id *note_oid)
{
	for (unsigned first = 0; first < 256; first++) {
		if (first == 0x80)
			continue;
		struct object_id key = subtree_prefix(first);
		if (note_tree_insert(notes, notes->root, 0,
				     leaf(&key, subtree_oid), PTR_TYPE_SUBTREE,
				     combine_notes_overwrite))
			die("could not insert fixture subtree");
	}
	for (unsigned second = 0; second < 256; second++) {
		struct object_id key = annotated_oid(0x80, second);
		if (note_tree_insert(notes, notes->root, 0,
				     leaf(&key, note_oid), PTR_TYPE_NOTE,
				     combine_notes_overwrite))
			die("could not insert fixture note");
	}
}

static double seconds_since(const struct timespec *start,
			    const struct timespec *end)
{
	return end->tv_sec - start->tv_sec +
		(end->tv_nsec - start->tv_nsec) / 1000000000.0;
}

static void replace_and_write(struct notes_tree *notes,
			      const struct object_id *annotated,
			      const struct object_id *note,
			      struct object_id *tree)
{
	if (add_note(notes, annotated, note, combine_notes_overwrite) ||
	    write_notes_tree(notes, tree))
		die("could not replace and write note");
}

int main(int argc, const char **argv)
{
	const uint64_t iterations = argc > 1 ? strtoull(argv[1], NULL, 10) : 20000;
	struct notes_tree notes = { 0 };
	struct object_id note_oid, replacement_oid, subtree_oid, annotated, tree;
	struct timespec start, end;
	uint64_t writes_before;
	double elapsed;

	if (!iterations)
		die("iteration count must be greater than zero");

	the_repository->hash_algo = &hash_algos[GIT_HASH_SHA1];
	the_repository->commondir = xstrdup("/tmp/git-notes-bench-no-objects");
	the_repository->objects = odb_new(the_repository, 0);
	odb_source_free(the_repository->objects->sources);
	the_repository->objects->sources = NULL;
	the_repository->objects->sources_tail = &the_repository->objects->sources;

	hash_object_file(the_repository->hash_algo, "note", 4, OBJ_BLOB,
			 &note_oid);
	hash_object_file(the_repository->hash_algo, "replacement", 11, OBJ_BLOB,
			 &replacement_oid);
	hash_object_file(the_repository->hash_algo, "opaque subtree", 14, OBJ_TREE,
			 &subtree_oid);
	annotated = annotated_oid(0x80, 0x80);

	CALLOC_ARRAY(notes.root, 1);
	notes.ref = "refs/notes/benchmark";
	notes.combine_notes = combine_notes_overwrite;
	notes.initialized = 1;
	insert_fixture(&notes, &subtree_oid, &note_oid);

	/* Match gix-note's reused-state benchmark: prime once with the replacement. */
	replace_and_write(&notes, &annotated, &replacement_oid, &tree);
	writes_before = object_writes;

	if (clock_gettime(CLOCK_MONOTONIC, &start))
		die_errno("clock_gettime");
	for (uint64_t i = 0; i < iterations; i++)
		replace_and_write(&notes, &annotated,
				  i & 1 ? &replacement_oid : &note_oid, &tree);
	if (clock_gettime(CLOCK_MONOTONIC, &end))
		die_errno("clock_gettime");

	elapsed = seconds_since(&start, &end);
	if (object_writes - writes_before != iterations * 258)
		die("expected 258 tree writes per replacement, got %.2f",
		    (double)(object_writes - writes_before) / iterations);

	printf("Git notes: %"PRIu64" PUTs in %.6f s = %.0f PUT/s (%.3f us/PUT); "
	       "258 tree hashes/PUT; final tree %s\n",
	       iterations, elapsed, iterations / elapsed,
	       elapsed * 1000000.0 / iterations, oid_to_hex(&tree));

	note_tree_free(notes.root);
	free(notes.root);
	odb_free(the_repository->objects);
	free(the_repository->commondir);
	return 0;
}
