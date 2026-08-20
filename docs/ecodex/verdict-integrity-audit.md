# Verdict-integrity audit

## Scope and method

The sweep covered every vendored-hook test, fresh-practice E2E, named Python/shell guard, curated workflow, release gate, and the prebuilt installer.

Each confirmation used a missing instrument, broken fixture, or opposing producer input. Of 16 candidates, 9 confirmed, 4 refuted, and 3 remained plausible: a 25% false-candidate rate.

## Confirmed and fixed

| Surface | Manufactured verdict demonstrated | Fix and retained failure path |
| --- | --- | --- |
| Vendored firewall drift guard | A file containing only the expected words in comments returned success. | Parse executable AST structure. Tests reject comment-only markers and a constant `permissionDecision`. |
| `setup-codex.py` source access | A nonexistent upstream ref produced an empty listing, “in sync,” and exit 0. | Git listing/blob failures and missing vendored targets now fail; the same invalid-ref probe now exits 2. |
| Hermetic vendored-hook CI | The exact CI environment returned `56 passed, 2 skipped`, exit 0; CLI parity and SQL schema validation had no live Empirica instrument. | CI installs pinned Empirica commit `f0e083c87523f8165c2623b568421ef079462d02`; parser/schema tests fail when it is absent. |
| Import-budget guard | A broken module import entered a `pytest.skip` branch, making the affected hook unable to fail its budget case. | Missing hooks and all module-load failures fail; a missing-dependency fixture pins the behavior. |
| Static SQL schema guard | Three unknown-table queries were counted as skips while the test passed. | Hook-created static tables are executed in the test schema; every other unknown table is a violation. The upstream-removed table is explicit and ratcheted. |
| Scoped cargo audit | `pretty_assertions@1.4.1` had a nonempty workspace inverse tree only through dev edges and was classified as shipped; the `codex-cli` normal/build inverse tree was empty. | Query each of the three release roots over normal/build edges across targets; tool failure remains fail-closed. |
| Prebuilt installer checksum | A valid archive plus a missing `.sha256` installed successfully. | Checksum download is mandatory. A fake-release fixture proves a missing checksum cannot install files. |
| Release install gate | Requesting `--verify-install` without `--create-gh-release` warned, skipped the requested check, and exited successfully. | The gate always attempts verification; an existing release is valid, and absence fails through the real installer. A fixture asserts the verifier ran. |
| Release version predicate | Output `ecodex 91.2.40` satisfied the substring check for expected `1.2.4`. | Require an exact whitespace-delimited version token; the adversarial output is retained as a regression test. |

## Refuted candidates

- Fresh-practice bootstrap requires both binaries and independently inspects persisted Git config; the missing-sandbox equivalence no longer reproduces.
- Release workflow `gh release create ... || true` is followed by a failing upload when creation genuinely fails and no release exists.
- `asciicheck.py --fix` returns nonzero after rewriting, conservatively requiring a clean rerun.
- Blob-size success on an empty diff is a measured empty input; added/renamed blobs remain discoverable and Git errors fail.

## Plausible limitations

- CLI parity ignores entirely dynamic Rust argv; concrete Python/Rust calls are asserted present, but the extractor remains a lower bound.
- Seven dynamic SQL expressions use internal identifier allow-lists but remain outside static SQLite validation.
- Optional remote publish operations can still warn and succeed when prerequisites are absent; their explicit-request semantics need separate review.

## Cross-repository finding

Vendored `tool-router.py` queries `epistemic_assessments`, removed upstream in favor of `reflexes`. It is now an explicit ratcheted known violation. The behavior fix belongs in Empirica core and was not patched here.
