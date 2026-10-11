"""Tests for the benchmark digest check and write commands."""

from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "sync_materialization_benchmark_digest.py"
_SPEC = importlib.util.spec_from_file_location("sync_materialization_benchmark_digest", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)


def _document(digest: str) -> str:
    return f"Its SHA-256 is `{digest}`.\n"


def _prepare(tmp_path: Path, monkeypatch: pytest.MonkeyPatch, text: str, mode: str) -> Path:
    artifact = tmp_path / "artifact.json"
    artifact.write_bytes(b"payload")
    document = tmp_path / "doc.md"
    document.write_text(text, encoding="utf-8")
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "sync_materialization_benchmark_digest.py",
            "--artifact",
            str(artifact),
            "--document",
            str(document),
            mode,
        ],
    )
    return document


def test_check_returns_zero_when_the_documented_digest_matches(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A document that already records the artifact digest passes with exit code 0."""
    digest = hashlib.sha256(b"payload").hexdigest()
    document = _prepare(tmp_path, monkeypatch, _document(digest), "--check")

    assert _MODULE.main() == 0
    assert document.read_text(encoding="utf-8") == _document(digest)


def test_check_mismatch_returns_one_without_rewriting(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """A digest mismatch is a failed check (exit 1), not a usage error (exit 2)."""
    stale = _document("0" * 64)
    document = _prepare(tmp_path, monkeypatch, stale, "--check")

    assert _MODULE.main() == 1
    assert document.read_text(encoding="utf-8") == stale
    assert "does not record artifact SHA-256" in capsys.readouterr().err


def test_write_records_the_artifact_digest(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Write mode replaces the documented digest and exits 0."""
    digest = hashlib.sha256(b"payload").hexdigest()
    document = _prepare(tmp_path, monkeypatch, _document("0" * 64), "--write")

    assert _MODULE.main() == 0
    assert document.read_text(encoding="utf-8") == _document(digest)
