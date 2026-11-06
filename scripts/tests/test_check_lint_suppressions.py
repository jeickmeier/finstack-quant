"""Tests for the forbidden lint-suppression check."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "check_lint_suppressions.py"
_SPEC = importlib.util.spec_from_file_location("check_lint_suppressions", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def test_flags_forbidden_suppressions_in_production_code(tmp_path: Path) -> None:
    """`allow` and `expect` of a forbidden lint fail; other lints do not."""
    _write(
        tmp_path / "finstack-quant/core/src/lib.rs",
        "#[allow(clippy::too_many_arguments)]\nfn a() {}\n"
        '#[expect(dead_code, reason = "x")]\nfn b() {}\n'
        "#[allow(clippy::module_inception)]\nmod c {}\n",
    )
    found = _MODULE.suppression_findings(tmp_path)
    assert found == [
        "finstack-quant/core/src/lib.rs:1: suppresses clippy::too_many_arguments",
        "finstack-quant/core/src/lib.rs:3: suppresses dead_code",
    ]


def test_ignores_test_only_items_and_test_files(tmp_path: Path) -> None:
    """Suppressions inside `#[cfg(test)]` items, `tests.rs` and `tests/` are allowed."""
    _write(
        tmp_path / "finstack-quant/core/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[allow(clippy::unwrap_used)]\n    fn t() {}\n}\n",
    )
    _write(tmp_path / "finstack-quant/core/src/money/tests.rs", "#[allow(dead_code)]\nfn t() {}\n")
    _write(tmp_path / "finstack-quant/core/src/money/tests/more.rs", "#[allow(dead_code)]\nfn t() {}\n")
    _write(tmp_path / "finstack-quant/core/tests/integration.rs", "#[allow(dead_code)]\nfn t() {}\n")
    assert _MODULE.suppression_findings(tmp_path) == []


def test_rejects_sibling_test_files_other_than_tests_rs(tmp_path: Path) -> None:
    """`foo_tests.rs` under `src/` fails; `tests.rs` and `tests/` do not."""
    _write(tmp_path / "finstack-quant/core/src/money/types_tests.rs", "fn t() {}\n")
    _write(tmp_path / "finstack-quant/core/src/money/tests/types_tests.rs", "fn t() {}\n")
    assert _MODULE.test_file_findings(tmp_path) == [
        "finstack-quant/core/src/money/types_tests.rs: rename to the module's tests.rs or move under tests/",
    ]


def test_report_mode_does_not_fail(tmp_path: Path, monkeypatch, capsys) -> None:
    """`--report` lists findings with exit 0; without it the same findings exit 1."""
    _write(tmp_path / "finstack-quant/core/src/lib.rs", "#[allow(unused_imports)]\nuse a::b;\n")
    monkeypatch.setattr(_MODULE, "__file__", str(tmp_path / "scripts" / "check_lint_suppressions.py"))
    assert _MODULE.main(["--report"]) == 0
    assert _MODULE.main([]) == 1
    assert "suppresses unused_imports" in capsys.readouterr().out


def test_backlog_tolerates_listed_entries_and_rejects_stale_ones() -> None:
    """A listed `path: lint` passes; an entry with no matching finding fails."""
    found = [
        "finstack-quant/core/src/a.rs:3: suppresses clippy::expect_used",
        "finstack-quant/core/src/b.rs:9: suppresses dead_code",
    ]
    backlog = {
        ("finstack-quant/core/src/a.rs", "clippy::expect_used"),
        ("finstack-quant/core/src/gone.rs", "dead_code"),
    }
    assert _MODULE.apply_backlog(found, backlog) == [
        "finstack-quant/core/src/b.rs:9: suppresses dead_code",
        "finstack-quant/core/src/gone.rs: backlog entry for dead_code matches no suppression;"
        " delete it from scripts/lint_suppressions_backlog.txt",
    ]
