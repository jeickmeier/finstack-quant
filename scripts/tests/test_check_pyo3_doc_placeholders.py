"""Tests for the public PyO3 runtime-documentation placeholder guard."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import sys

_SCRIPT_PATH = Path(__file__).parents[1] / "check_pyo3_doc_placeholders.py"
_SPEC = importlib.util.spec_from_file_location("check_pyo3_doc_placeholders", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)


def test_public_pyo3_placeholders_are_rejected(tmp_path: Path) -> None:
    """Class, method, and free-function runtime docs reject every legacy pattern."""
    fixture = tmp_path / "placeholder.rs"
    fixture.write_text(
        """/// Represent one widget.
///
/// Examples
/// --------
/// >>> Widget.__name__
/// 'Widget'
#[pyclass]
struct Widget;

#[pymethods]
impl Widget {
    /// Create a widget.
    ///
    /// Examples
    /// --------
    /// >>> callable(Widget.create)
    /// True
    #[staticmethod]
    fn create() -> Self {
        Self
    }
}

/// Compute a value.
///
/// Value supplied for `value` to the documented binding operation.
/// Result of compute for the binding in the annotated representation.
/// Validated `Widget` instance reconstructed from the canonical JSON payload.
/// If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
/// This method does not raise; undefined results use ``None``, ``NaN``, or ``inf`` rather than an exception.
///
/// Examples
/// --------
/// >>> compute(2)  # doctest: +SKIP
/// 4
#[pyfunction]
fn compute(value: i32) -> i32 {
    value * 2
}
""",
        encoding="utf-8",
    )

    assert [(error.owner, error.message) for error in _MODULE.placeholder_errors(fixture)] == [
        ("Widget", "name-only doctest"),
        ("create", "callability-only doctest"),
        ("compute", "skipped doctest"),
        ("compute", "fabricated parameter description"),
        ("compute", "fabricated return description"),
        ("compute", "fabricated reconstruction description"),
        ("compute", "fabricated error description"),
        ("compute", "generic-nan-sentinel boilerplate"),
    ]


def test_non_public_and_non_doc_text_is_ignored(tmp_path: Path) -> None:
    """Runtime strings, ordinary comments, and private Rust docs stay out of scope."""
    fixture = tmp_path / "private.rs"
    fixture.write_text(
        """const RUNTIME_TEXT: &str = ">>> callable(Widget.create)";
// >>> Widget.__name__

/// >>> callable(private_helper)
fn private_helper() {}

/// Double one value.
///
/// Examples
/// --------
/// >>> double(2)
/// 4
#[pyfunction]
fn double(value: i32) -> i32 {
    value * 2
}
""",
        encoding="utf-8",
    )

    assert [block.owner for block in _MODULE.public_pyo3_doc_blocks(fixture)] == ["double"]
    assert _MODULE.placeholder_errors(fixture) == []


def test_method_generating_macro_entries_are_public(tmp_path: Path) -> None:
    """Docs forwarded by `py_getters!` / Greek / envelope macros are checked."""
    fixture = tmp_path / "macros.rs"
    fixture.write_text(
        """crate::bindings::macros::py_getters!(PyWidget |s| {
    /// Widget size.
    ///
    /// >>> callable(Widget.size)
    size -> f64 = s.inner.size;
});

instrument_greek_methods!(PyOption {
    /// Delta of the option.
    delta,
});

instrument_envelope_methods!(
    PyOption,
    OptionVariant,
    "option",
    builder = PyOptionBuilder("OptionBuilder", fields),
    seed = Option::builder(),
    /// Build the validated option.
    build
);
""",
        encoding="utf-8",
    )

    assert [block.owner for block in _MODULE.public_pyo3_doc_blocks(fixture)] == [
        "size",
        "delta",
        "build",
    ]
    assert [(error.owner, error.message) for error in _MODULE.placeholder_errors(fixture)] == [
        ("size", "callability-only doctest"),
    ]
