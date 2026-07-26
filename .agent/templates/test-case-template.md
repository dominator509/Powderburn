# Test case: <name>

Spec behavior: SPEC-XXX section N, statement M
Level: unit | integration | contract | e2e | live-fire
File: crates/<crate>/tests/<file>.rs

## Real dependencies used
Name them. If any is a double, say why the zone in TESTING.md permits it.

## Setup
Exact scenario, seed, content files, and directory under $PB_CACHE_DIR/test/<name>/.

## Action
The exact call or command.

## Assertions
What must be true, stated as observable values, not as feelings about the output.

## Failure proof
The change that, made to the implementation, causes this test to fail. Verified once at authoring
time. A test whose failure mode has never been observed is not yet a test.

## Cleanup
Exact removal, on success and on failure.
