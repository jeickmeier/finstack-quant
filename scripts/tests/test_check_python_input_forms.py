"""Tests for the Python input-form guard (a ``str`` is a value, not JSON text)."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "check_python_input_forms.py"
_SPEC = importlib.util.spec_from_file_location("check_python_input_forms", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)

_LOADABLE = """class Spec:
    @classmethod
    def from_json(cls, text: str) -> Spec: ...

class Book:
    @staticmethod
    def from_spec(spec: str) -> Book: ...

class Plain:
    def __init__(self, name: str) -> None: ...
"""


def _stubs(tmp_path: Path, functions: str) -> Path:
    root = tmp_path / "pkg"
    root.mkdir()
    (root / "types.pyi").write_text(_LOADABLE, encoding="utf-8")
    (root / "api.pyi").write_text(functions, encoding="utf-8")
    return root


def _exempt_nothing(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(_MODULE, "LABEL_CLASSES", {})
    monkeypatch.setattr(_MODULE, "TEXT_PARAMETERS", {})


def test_str_beside_a_loadable_class_is_reported(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Every union form is read: ``|``, ``Optional``, ``Union``, nested and quoted."""
    _exempt_nothing(monkeypatch)
    root = _stubs(
        tmp_path,
        """from typing import Optional, Sequence, Union

def bar_union(spec: Spec | str) -> None: ...
def optional(spec: Optional[Spec | str] = None) -> None: ...
def typing_union(book: Union[Book, str, None]) -> None: ...
def nested(specs: Sequence[Spec | str]) -> None: ...
def quoted(spec: "Spec | str") -> None: ...

class Runner:
    def run(self, *, spec: str | Spec) -> None: ...
""",
    )

    diagnostics = _MODULE.check(root)

    assert [line.split(": ", 1)[1].split(" takes ")[0] for line in diagnostics] == [
        "bar_union(spec)",
        "optional(spec)",
        "typing_union(book)",
        "nested(specs)",
        "quoted(spec)",
        "Runner.run(spec)",
    ]
    assert "`Spec | str`" in diagnostics[0]
    assert "`Book | str`" in diagnostics[2]


def test_values_and_separate_unions_are_not_reported(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A label, a class with no loader, and ``str`` in a different union are fine."""
    _exempt_nothing(monkeypatch)
    root = _stubs(
        tmp_path,
        """def label(name: str, code: str | None = None) -> None: ...
def typed(spec: Spec, book: Book | None = None) -> None: ...
def no_loader(plain: Plain | str) -> None: ...
def separate(specs: dict[str, Spec], tags: list[str] | None = None) -> None: ...
def returns_text(spec: Spec) -> Spec | str: ...
""",
    )

    assert _MODULE.check(root) == []


def test_exemptions_apply_and_stale_ones_are_errors(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A label class and a text parameter are exempt; an unused entry is reported."""
    root = _stubs(
        tmp_path,
        """def label(spec: Spec | str) -> None: ...
def validate(book: Book | str) -> None: ...
""",
    )
    monkeypatch.setattr(_MODULE, "LABEL_CLASSES", {"Spec": "a label"})
    monkeypatch.setattr(_MODULE, "TEXT_PARAMETERS", {("api.pyi", "validate", "book"): "the text is the subject"})
    assert _MODULE.check(root) == []

    monkeypatch.setattr(_MODULE, "LABEL_CLASSES", {"Spec": "a label", "Gone": "unused"})
    monkeypatch.setattr(
        _MODULE,
        "TEXT_PARAMETERS",
        {("api.pyi", "validate", "book"): "the text is the subject", ("api.pyi", "removed", "x"): "unused"},
    )
    assert _MODULE.check(root) == [
        "LABEL_CLASSES entry 'Gone' matches no parameter; remove it",
        "TEXT_PARAMETERS entry ('api.pyi', 'removed', 'x') matches no parameter; remove it",
    ]


def test_main_reports_and_sets_the_exit_code(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """The command prints each finding to stderr and exits 1; a clean tree exits 0."""
    _exempt_nothing(monkeypatch)
    root = _stubs(tmp_path, "def run(spec: Spec | str) -> None: ...\n")

    assert _MODULE.main(["--stub-root", str(root)]) == 1
    assert "run(spec) takes `Spec | str`" in capsys.readouterr().err

    (root / "api.pyi").write_text("def run(spec: Spec) -> None: ...\n", encoding="utf-8")
    assert _MODULE.main(["--stub-root", str(root)]) == 0
    assert "clean (2 stub files)" in capsys.readouterr().out


def test_the_repository_stubs_are_clean() -> None:
    """The checked-in stubs pass, and every exemption in the script is still used."""
    assert _MODULE.check(_MODULE.STUB_ROOT) == []
