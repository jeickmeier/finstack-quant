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

_CLASSES = """class Spec:
    @classmethod
    def from_json(cls, text: str) -> Spec: ...

class Book:
    @staticmethod
    def from_spec(spec: str) -> Book: ...

class Params:
    def __init__(self, rate: float) -> None: ...

class Kind:
    @classmethod
    def from_name(cls, name: str) -> Kind: ...
"""


def _stubs(tmp_path: Path, functions: str) -> Path:
    root = tmp_path / "pkg"
    root.mkdir()
    (root / "types.pyi").write_text(_CLASSES, encoding="utf-8")
    (root / "api.pyi").write_text(functions, encoding="utf-8")
    return root


def _exempt_nothing(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(_MODULE, "LABEL_CLASSES", {})
    monkeypatch.setattr(_MODULE, "TEXT_PARAMETERS", {})


def _reported(diagnostics: list[str]) -> list[str]:
    return [line.split(": ", 1)[1].split(" takes ")[0] for line in diagnostics]


def test_str_beside_a_package_class_is_reported(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Every union form is read: ``|``, ``Optional``, ``Union``, nested and quoted."""
    _exempt_nothing(monkeypatch)
    root = _stubs(
        tmp_path,
        """from typing import Any, Optional, Sequence, Union

def bar_union(spec: Spec | str) -> None: ...
def optional(spec: Optional[Spec | str] = None) -> None: ...
def typing_union(book: Union[Book, str, None]) -> None: ...
def nested(specs: Sequence[Spec | str]) -> None: ...
def quoted(spec: "Spec | str") -> None: ...
def no_loader(params: Params | dict[str, Any] | str) -> None: ...

class Runner:
    def run(self, *, spec: str | Spec) -> None: ...
""",
    )

    diagnostics = _MODULE.check(root)

    assert _reported(diagnostics) == [
        "bar_union(spec)",
        "optional(spec)",
        "typing_union(book)",
        "nested(specs)",
        "quoted(spec)",
        "no_loader(params)",
        "Runner.run(spec)",
    ]
    assert "takes `str` beside Spec;" in diagnostics[0]
    assert "takes `str` beside Book;" in diagnostics[2]
    assert "takes `str` beside Params;" in diagnostics[5]


def test_type_aliases_are_expanded(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A class hidden behind a module-level alias is still seen, in every alias spelling."""
    _exempt_nothing(monkeypatch)
    root = _stubs(
        tmp_path,
        """from typing import Any, TypeAlias

_Input = dict[str, Any] | Spec | Book
_Annotated: TypeAlias = Params | None
type _Modern = Kind | int
_Outer = _Input | float
_Loop = _Loop | str

def plain(value: _Input | str) -> None: ...
def annotated(value: _Annotated | str) -> None: ...
def modern(value: _Modern | str) -> None: ...
def chained(value: list[_Outer | str]) -> None: ...
def alone(value: _Input) -> None: ...
def looped(value: _Loop) -> None: ...
""",
    )

    diagnostics = _MODULE.check(root)

    assert _reported(diagnostics) == ["plain(value)", "annotated(value)", "modern(value)", "chained(value)"]
    assert "takes `str` beside Book, Spec;" in diagnostics[0]
    assert "takes `str` beside Params;" in diagnostics[1]
    assert "takes `str` beside Kind;" in diagnostics[2]


def test_values_and_separate_unions_are_not_reported(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A label, a builtin beside ``str``, and ``str`` in a different union are fine."""
    _exempt_nothing(monkeypatch)
    root = _stubs(
        tmp_path,
        """import datetime
from typing import Any

def label(name: str, code: str | None = None) -> None: ...
def typed(spec: Spec, book: Book | None = None) -> None: ...
def builtins(as_of: datetime.date | str, value: float | int | str) -> None: ...
def mapping(payload: dict[str, Any] | str) -> None: ...
def separate(specs: dict[str, Spec], tags: list[str] | None = None) -> None: ...
def returns_text(spec: Spec) -> Spec | str: ...
""",
    )

    assert _MODULE.check(root) == []


def test_exemptions_apply_and_stale_ones_are_errors(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A label class and a text parameter are exempt; an unused entry is reported."""
    root = _stubs(
        tmp_path,
        """def label(kind: Kind | str) -> None: ...
def mixed(value: Kind | Spec | str) -> None: ...
def validate(book: Book | str) -> None: ...
""",
    )
    monkeypatch.setattr(_MODULE, "LABEL_CLASSES", {"Kind": "a variant name"})
    monkeypatch.setattr(_MODULE, "TEXT_PARAMETERS", {("api.pyi", "validate", "book"): "the text is the subject"})
    diagnostics = _MODULE.check(root)
    assert _reported(diagnostics) == ["mixed(value)"]
    assert "takes `str` beside Spec;" in diagnostics[0]

    monkeypatch.setattr(_MODULE, "LABEL_CLASSES", {"Kind": "a variant name", "Spec": "exempt here", "Gone": "unused"})
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
    assert "run(spec) takes `str` beside Spec;" in capsys.readouterr().err

    (root / "api.pyi").write_text("def run(spec: Spec) -> None: ...\n", encoding="utf-8")
    assert _MODULE.main(["--stub-root", str(root)]) == 0
    assert "clean (2 stub files)" in capsys.readouterr().out


def test_the_repository_stubs_are_clean() -> None:
    """The checked-in stubs pass, and every exemption in the script is still used."""
    assert _MODULE.check(_MODULE.STUB_ROOT) == []
