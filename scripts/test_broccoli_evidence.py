"""Tests for scripts/broccoli_evidence.py (broccoli.sh's Empirica evidence files)."""

from __future__ import annotations

import importlib.util
import json
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "broccoli_evidence", Path(__file__).with_name("broccoli_evidence.py")
)
assert _SPEC is not None and _SPEC.loader is not None
be = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(be)


def _diag(level, message, line=10, code="clippy::needless_return", target="lib"):
    return json.dumps(
        {
            "reason": "compiler-message",
            "target": {"kind": [target]},
            "message": {
                "level": level,
                "message": message,
                "code": {"code": code} if code else None,
                "spans": [
                    {
                        "file_name": "src/lib.rs",
                        "line_start": line,
                        "column_start": 5,
                        "is_primary": True,
                    }
                ],
                "rendered": f"{level}: {message}\n",
            },
        }
    )


FINISHED_OK = json.dumps({"reason": "build-finished", "success": True})
FINISHED_FAIL = json.dumps({"reason": "build-finished", "success": False})
SUMMARY = json.dumps(
    {
        "reason": "compiler-message",
        "message": {"level": "warning", "message": "1 warning emitted", "code": None, "spans": []},
    }
)


def test_clippy_counts_a_diagnostic_once_across_targets_and_skips_summaries():
    stream = "\n".join(
        [
            _diag("warning", "unneeded return"),
            _diag("warning", "unneeded return", target="test"),
            SUMMARY,
            _diag("warning", "unused variable", line=20, code="unused_variables"),
            FINISHED_OK,
        ]
    )
    counts, rendered = be.clippy_counts(stream)
    assert counts == {"warnings": 2, "errors": 0}
    assert len(rendered) == 2


def test_clippy_clean_build_counts_zero():
    assert be.clippy_counts(json.dumps({"reason": "compiler-artifact"}) + "\n" + FINISHED_OK)[
        0
    ] == {"warnings": 0, "errors": 0}


def test_clippy_without_a_finished_build_measured_nothing():
    assert be.clippy_counts(_diag("warning", "x"))[0] is None
    assert be.clippy_counts(FINISHED_FAIL)[0] is None  # failed with no diagnostic to explain it


def test_clippy_compile_errors_are_counted():
    counts, _ = be.clippy_counts(
        "\n".join([_diag("error", "mismatched types", code="E0308"), FINISHED_FAIL])
    )
    assert counts == {"warnings": 0, "errors": 1}


CARGO_TEST = """
     Running unittests src/lib.rs (target/debug/deps/codex_empirica_plugin-0a1b2c3d)
test result: ok. 40 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.20s

     Running tests/all.rs (target/debug/deps/all-4e5f6a7b)
test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.02s

   Doc-tests codex_empirica_plugin
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
"""


def test_cargo_test_counts_sum_every_binary():
    assert be.cargo_test_counts(CARGO_TEST, 101) == {
        "passed": 52,
        "failed": 1,
        "ignored": 2,
        "filtered_out": 0,
        "binaries": 3,
    }


def test_cargo_test_failure_without_a_failing_test_measured_nothing():
    assert be.cargo_test_counts("error[E0425]: cannot find value `x`\n", 101) is None
    ok_then_crash = (
        "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        "error: test failed, SIGSEGV\n"
    )
    assert be.cargo_test_counts(ok_then_crash, 101) is None


def test_emit_writes_atomically_and_removes_on_nothing_measured(tmp_path):
    target = tmp_path / "test_report.json"
    assert "wrote" in be.emit(target, {"scope": "owned", "summary": {"passed": 1, "failed": 0}})
    report = json.loads(target.read_text())
    assert report["runner"] == "broccoli.sh" and report["summary"] == {"passed": 1, "failed": 0}
    assert isinstance(report["created"], float)
    assert list(tmp_path.iterdir()) == [target]  # no temp file left behind
    assert "not written" in be.emit(target, None)
    assert not target.exists()


def test_emit_skips_a_missing_directory(tmp_path):
    assert "does not exist" in be.emit(tmp_path / "absent" / "code_quality.json", {"metrics": {}})


def test_main_clippy_gate_and_file(tmp_path):
    log = tmp_path / "clippy.json"
    log.write_text("\n".join([_diag("warning", "unneeded return"), FINISHED_OK]))
    out = tmp_path / "code_quality.json"
    status = be.main(
        [
            "clippy",
            str(log),
            "--cargo-status",
            "0",
            "--rustfmt-ok",
            "true",
            "--scope",
            "owned crates",
            "--out",
            str(out),
        ]
    )
    assert status == 1  # a warning fails the gate even though cargo exited 0
    assert json.loads(out.read_text())["metrics"] == {
        "rustfmt_ok": True,
        "clippy_warnings": 1,
        "clippy_errors": 0,
    }


def test_main_tests_gate_follows_cargo(tmp_path):
    log = tmp_path / "test.log"
    log.write_text(CARGO_TEST.replace("FAILED. 9 passed; 1 failed", "ok. 10 passed; 0 failed"))
    out = tmp_path / "test_report.json"
    assert (
        be.main(
            ["tests", str(log), "--cargo-status", "0", "--scope", "owned crates", "--out", str(out)]
        )
        == 0
    )
    assert json.loads(out.read_text())["summary"]["passed"] == 53


OWNED = {"codex-empirica-plugin", "codex-empirica-translator", "codex-cli"}


def _pkg_diag(package_id, message="unused import", line=3):
    d = json.loads(_diag("warning", message, line=line, code="unused_imports"))
    d["package_id"] = package_id
    return json.dumps(d)


def test_package_name_from_both_package_id_formats():
    assert (
        be.package_name("path+file:///x/codex-rs/codex-empirica-translator#0.164.2")
        == "codex-empirica-translator"
    )
    assert be.package_name("path+file:///x/codex-rs/cli#codex-cli@0.164.2") == "codex-cli"
    assert be.package_name("codex-core 0.164.2 (path+file:///x/codex-rs/core)") == "codex-core"


def test_clippy_counts_only_the_owned_packages():
    # With --no-deps, clippy lints only the named packages, but cargo still prints rustc warnings
    # from workspace dependencies.
    stream = "\n".join(
        [
            _pkg_diag("path+file:///x/codex-rs/core#codex-core@0.164.2"),
            _pkg_diag("path+file:///x/codex-rs/cli#codex-cli@0.164.2", line=9),
            FINISHED_OK,
        ]
    )
    assert be.clippy_counts(stream, OWNED)[0] == {"warnings": 1, "errors": 0}


def test_clippy_counts_a_warning_without_a_span():
    unknown_lint = json.dumps(
        {
            "reason": "compiler-message",
            "message": {
                "level": "warning",
                "message": "unknown lint: `clippy::no_such_lint`",
                "code": {"code": "unknown_lints"},
                "spans": [],
                "rendered": "warning: unknown lint\n",
            },
        }
    )
    assert be.clippy_counts("\n".join([unknown_lint, SUMMARY, FINISHED_OK]))[0] == {
        "warnings": 1,
        "errors": 0,
    }


def test_a_crashed_binary_beside_a_failing_one_measured_nothing():
    crashed = (
        "test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n"
        "error: test failed, to rerun pass `-p codex-empirica-plugin --lib`\n"
        "error: test failed, to rerun pass `-p codex-empirica-plugin --test all`\n\nCaused by:\n"
        "  process didn't exit successfully: `/x/all-1a2b` (signal: 11, SIGSEGV: invalid memory "
        "reference)\n"
        "error: 2 targets failed:\n    `-p codex-empirica-plugin --lib`\n    `-p "
        "codex-empirica-plugin --test all`\n"
    )
    assert be.cargo_test_counts(crashed, 101) is None
    one_failure = (
        "test result: FAILED. 9 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n"
        "error: test failed, to rerun pass `-p codex-empirica-plugin --lib`\n"
        "error: 1 target failed:\n    `-p codex-empirica-plugin --lib`\n"
    )
    assert be.cargo_test_counts(one_failure, 101)["failed"] == 1
