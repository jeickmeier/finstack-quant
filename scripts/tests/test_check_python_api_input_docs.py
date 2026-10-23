"""Tests for Python API documentation policy."""

from __future__ import annotations

import importlib.util
from pathlib import Path

_SCRIPT_PATH = Path(__file__).parents[1] / "check_python_api_input_docs.py"
_SPEC = importlib.util.spec_from_file_location("check_python_api_input_docs", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_MODULE)


def test_parameterized_nonthrowing_callable_does_not_require_raises(tmp_path: Path) -> None:
    """Parameters alone must not fabricate an exception contract."""
    fixture = tmp_path / "documented.pyi"
    fixture.write_text(
        '''"""Documented fixture module.

Examples
--------
>>> scale(2.0)
4.0
"""

def scale(value: float) -> float:
    """Scale a finite value by two.

    Parameters
    ----------
    value : float
        Finite scalar to multiply by two.

    Returns
    -------
    float
        Twice the supplied scalar.

    Notes
    -----
    This helper does not raise for documented inputs.

    Examples
    --------
    >>> scale(2.0)
    4.0
    """
    ...
''',
        encoding="utf-8",
    )

    assert _MODULE.public_callable_errors(fixture) == []


def test_module_level_parameterized_callable_requires_exception_behavior(tmp_path: Path) -> None:
    """Module-level helpers must document Raises or that they do not raise."""
    fixture = tmp_path / "undocumented.pyi"
    fixture.write_text(
        '''"""Documented fixture module.

Examples
--------
>>> scale(2.0)
4.0
"""

def scale(value: float) -> float:
    """Scale a finite value by two.

    Parameters
    ----------
    value : float
        Finite scalar to multiply by two.

    Returns
    -------
    float
        Twice the supplied scalar.

    Examples
    --------
    >>> scale(2.0)
    4.0
    """
    ...
''',
        encoding="utf-8",
    )

    assert [error.message for error in _MODULE.public_callable_errors(fixture)] == [
        "callable without Raises must document that it does not raise",
    ]


def test_parameterized_none_return_needs_neither_returns_nor_raises(tmp_path: Path) -> None:
    """A documented command may return and raise nothing."""
    fixture = tmp_path / "command.pyi"
    fixture.write_text(
        '''"""Documented command module.

Examples
--------
>>> record("trade")
"""

def record(name: str) -> None:
    """Record one trade identifier.

    Parameters
    ----------
    name : str
        Non-empty trade identifier to record.

    Notes
    -----
    This helper does not raise for a non-empty identifier.

    Examples
    --------
    >>> record("trade")
    """
    ...
''',
        encoding="utf-8",
    )

    assert _MODULE.public_callable_errors(fixture) == []


def test_instance_method_requires_exception_behavior(tmp_path: Path) -> None:
    """Instance methods must document Raises or that they do not raise."""
    fixture = tmp_path / "instance.pyi"
    fixture.write_text(
        '''"""Documented instance-method module.

Examples
--------
>>> Doubler().apply(2.0)
4.0
"""

class Doubler:
    """Multiply scalar inputs by two.

    Examples
    --------
    >>> Doubler().apply(2.0)
    4.0
    """

    def apply(self, value: float) -> float:
        """Multiply one finite scalar by two.

        Parameters
        ----------
        value : float
            Finite scalar to multiply by two.

        Returns
        -------
        float
            Twice the supplied scalar.
        """
        ...
''',
        encoding="utf-8",
    )

    assert [error.message for error in _MODULE.public_callable_errors(fixture)] == [
        "callable without Raises must document that it does not raise",
    ]


def test_class_example_covers_ordinary_instance_method(tmp_path: Path) -> None:
    """Routine accessors share the class-level usage example."""
    fixture = tmp_path / "class_api.pyi"
    fixture.write_text(
        '''"""Documented class module.

Examples
--------
>>> Doubler().apply(2.0)
4.0
"""

class Doubler:
    """Multiply scalar inputs by two.

    Examples
    --------
    >>> Doubler().apply(2.0)
    4.0
    """

    def apply(self, value: float) -> float:
        """Multiply one finite scalar by two.

        Parameters
        ----------
        value : float
            Finite scalar to multiply by two.

        Returns
        -------
        float
            Twice the supplied scalar.

        Notes
        -----
        This method does not raise; it returns twice the supplied finite scalar.
        """
        ...
''',
        encoding="utf-8",
    )

    assert _MODULE.public_callable_errors(fixture) == []


def test_import_and_introspection_placeholders_are_rejected(tmp_path: Path) -> None:
    """Generated name and callability checks are not usage examples."""
    fixture = tmp_path / "placeholder_api.pyi"
    fixture.write_text(
        '''"""Placeholder fixture module.

Examples
--------
>>> import placeholder_api
>>> placeholder_api.__name__
'placeholder_api'
"""

class Widget:
    """Represent one documented widget.

    Examples
    --------
    >>> from placeholder_api import Widget
    >>> Widget.__name__
    'Widget'
    """

    @staticmethod
    def create() -> Widget:
        """Create a widget with the documented defaults.

        Returns
        -------
        Widget
            Newly allocated widget using the documented defaults.

        Notes
        -----
        This factory does not raise; it returns a widget with the documented defaults.

        Examples
        --------
        >>> from placeholder_api import Widget
        >>> callable(Widget.create)
        True
        """
        ...
''',
        encoding="utf-8",
    )

    assert [error.message for error in _MODULE.public_callable_errors(fixture)] == [
        "placeholder module usage example",
        "placeholder class usage example",
        "placeholder callable usage example",
    ]


def test_real_call_after_import_is_substantive(tmp_path: Path) -> None:
    """Imports may set up an example when an actual API call follows."""
    fixture = tmp_path / "real_api.pyi"
    fixture.write_text(
        '''"""Real fixture module.

Examples
--------
>>> from real_api import scale
>>> scale(2.0)
4.0
"""

def scale(value: float) -> float:
    """Scale a finite value by two.

    Parameters
    ----------
    value : float
        Finite scalar to multiply by two.

    Returns
    -------
    float
        Twice the supplied scalar.

    Notes
    -----
    This helper does not raise for documented inputs.

    Examples
    --------
    >>> from real_api import scale
    >>> scale(2.0)
    4.0
    """
    ...
''',
        encoding="utf-8",
    )

    assert _MODULE.public_callable_errors(fixture) == []


def test_type_introspection_placeholder_is_rejected(tmp_path: Path) -> None:
    """Checking that a public class object is a type is not class usage."""
    fixture = tmp_path / "type_placeholder.pyi"
    fixture.write_text(
        '''"""Type-placeholder fixture module.

Examples
--------
>>> Widget(2).value
2
"""

class Widget:
    """Store one integer widget value.

    Examples
    --------
    >>> isinstance(Widget, type)
    True
    """

    def __init__(self, value: int) -> None:
        """Create a widget from one integer.

        Parameters
        ----------
        value : int
            Integer payload retained by the widget.

        Notes
        -----
        Construction does not raise; the integer is stored as supplied.
        """
        ...

    @property
    def value(self) -> int:
        """Return the retained integer payload.

        Returns
        -------
        int
            Integer supplied when the widget was constructed.

        Notes
        -----
        This accessor does not raise; it returns the stored integer.
        """
        ...
''',
        encoding="utf-8",
    )

    assert [error.message for error in _MODULE.public_callable_errors(fixture)] == ["placeholder class usage example"]


def test_generator_boilerplate_patterns_are_rejected() -> None:
    """Every known false generator template receives a stable diagnostic."""
    docstring = """Compute   init for `Widget`.

    Value supplied for ``value`` to the documented binding operation.
    Result of create for this Widget in the annotated representation.
    If the JSON payload cannot be parsed or does not satisfy the `ValueError` schema and invariants.
    If supplied inputs violate the documented type, shape, finite-value, or domain constraints.
    Stable identifier used to select the required object or result entry.
    Date used in the documented calculation or scheduling role.
    Ordered input values consumed by the calculation in the documented representation.
    Supported selector string or enum value controlling the documented behavior.
    Value of ``to_json``.
    Compute VarianceRow.
    The id exposed by this `Widget`.
    Compute daily for `Tenor`.
    Return the id for `Widget`.
    This method does not raise; undefined results use ``None``, ``NaN``, or ``inf`` rather than an exception.
    """

    assert _MODULE.fabricated_doc_messages(docstring) == [
        "annotated-representation boilerplate",
        "binding-operation parameter boilerplate",
        "constructor-summary boilerplate",
        "ValueError-schema boilerplate",
        "unspecified-constraint boilerplate",
        "generic-identifier boilerplate",
        "generic-date boilerplate",
        "generic-sequence boilerplate",
        "generic-selector boilerplate",
        "value-of-name boilerplate",
        "compute-typename boilerplate",
        "generic-field-exposed boilerplate",
        "generic-compute-for boilerplate",
        "generic-return-the-for boilerplate",
        "generic-nan-sentinel boilerplate",
    ]


def test_specific_documentation_is_not_rejected_as_boilerplate() -> None:
    """Concrete identifiers, dates, conventions, and constraints remain valid."""
    docstring = """Build a dated curve for one exact identifier.

    ``id`` is the exact market-context lookup key. ``base_date`` is the curve
    valuation date. Values are expiry-major decimal rates. Invalid calendar
    dates raise ``ValueError`` and an unsupported convention raises ``KeyError``.
    """

    assert _MODULE.fabricated_doc_messages(docstring) == []


_NATIVE_DELEGATE = "from finstack_quant.finstack_quant import models as _models\n__doc__ = _models.__doc__\n"
_VALID_STUB_MODULE = '"""Reusable analytical pricing models.\n\nExamples\n--------\n>>> round(2.0)\n2\n"""\n'


def test_native_module_doc_delegation_uses_matching_stub(tmp_path: Path) -> None:
    """Binding shims validate their canonical stub module documentation."""
    fixture = tmp_path / "models.py"
    fixture.write_text(_NATIVE_DELEGATE)
    fixture.with_suffix(".pyi").write_text(_VALID_STUB_MODULE)
    assert _MODULE.public_callable_errors(fixture) == []


def test_native_module_doc_delegation_requires_both_delegate_and_stub(tmp_path: Path) -> None:
    """A nearby stub does not excuse an undocumented ordinary Python module."""
    fixture = tmp_path / "models.py"
    fixture.write_text(_NATIVE_DELEGATE)
    assert any(error.message == "missing module docstring" for error in _MODULE.public_callable_errors(fixture))
    fixture.with_suffix(".pyi").write_text(_VALID_STUB_MODULE)
    fixture.write_text("from finstack_quant.finstack_quant import models as _models\n")
    assert any(error.message == "missing module docstring" for error in _MODULE.public_callable_errors(fixture))
    fixture.write_text("import unrelated as _models\n__doc__ = _models.__doc__\n")
    assert any(error.message == "missing module docstring" for error in _MODULE.public_callable_errors(fixture))


def test_delegated_module_docs_still_validate_content_and_local_functions(tmp_path: Path) -> None:
    """Delegation preserves summary, example and callable checks."""
    fixture = tmp_path / "models.py"
    fixture.write_text(_NATIVE_DELEGATE)
    fixture.with_suffix(".pyi").write_text('"""API."""\n')
    assert _MODULE.public_callable_errors(fixture)
    fixture.with_suffix(".pyi").write_text(_VALID_STUB_MODULE)
    fixture.write_text(_NATIVE_DELEGATE + "def scale(value):\n    return value * 2\n")
    errors = _MODULE.public_callable_errors(fixture)
    assert any(error.symbol == "scale" and error.message == "missing callable docstring" for error in errors)
