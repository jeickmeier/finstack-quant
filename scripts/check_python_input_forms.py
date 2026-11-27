"""Keep JSON text out of the Python API except at its named loaders and ``to_json``.

In the Python API a ``str`` argument is a value: a label, a code, a date, an id.
It is never a JSON document. JSON text enters through a named loader
(``T.from_json``, ``Portfolio.from_spec``) or ``json.loads``, and the text of a
result comes from ``to_json()``.

This check reads the ``.pyi`` stubs under ``finstack-quant-py/finstack_quant``
and applies two rules.

Input. A parameter whose annotation holds ``str`` in the same union as a class
the package defines offers two ways to pass one value, so it fails.
Module-level type aliases are expanded first. Two lists are exempt: the classes
for which a string is a label, and the parameters whose subject is the JSON
text itself.

Output. A public function or member named ``*_json`` (other than ``to_json``
and ``from_json``) is a text twin of a typed form, so it fails unless it is
listed in ``JSON_NAMED``.

Every exempt entry carries its reason, and an entry that no longer matches
anything is also an error, so the lists cannot go stale.

The check sees names and annotations only, and it does not judge
``dict | str``, where the string is a label for some parameters.
``finstack-quant-py/tests/test_json_text_inputs.py`` proves the runtime
behaviour.
"""

from __future__ import annotations

import argparse
import ast
from collections.abc import Iterator, Sequence
from dataclasses import dataclass
from pathlib import Path
import sys

REPO_ROOT = Path(__file__).resolve().parents[1]
STUB_ROOT = REPO_ROOT / "finstack-quant-py" / "finstack_quant"

_VARIANT = "a variant name"

# Classes whose string form is a label or a number in text, not a document.
LABEL_CLASSES: dict[str, str] = {
    "Bps": "a basis-point amount in text",
    "BusinessDayConvention": _VARIANT,
    "CFKind": _VARIANT,
    "CollateralAssetClass": _VARIANT,
    "Compounding": _VARIANT,
    "CreditRating": "a rating label such as 'BBB+'",
    "CrossSectionalOp": _VARIANT,
    "Currency": "an ISO 4217 code",
    "CurveKind": _VARIANT,
    "DayCount": "a day-count name such as 'act_365f'",
    "FeeAccrualBasis": _VARIANT,
    "FeeBase": "the label of a fee base without parameters",
    "FloatingLegCompounding": _VARIANT,
    "FloatingRateFallback": _VARIANT,
    "FxConversionPolicy": _VARIANT,
    "GridMode": _VARIANT,
    "HolidayCalendar": "a calendar code such as 'nyse'",
    "ImMethodology": _VARIANT,
    "LeaseGrowthConvention": _VARIANT,
    "Money": "a decimal amount in text",
    "NodeType": _VARIANT,
    "OvernightIndexConstraintApplication": _VARIANT,
    "PairwiseOp": _VARIANT,
    "Rate": "a decimal rate in text",
    "RiskMeasure": "one of the two measures without parameters",
    "RollRule": _VARIANT,
    "RoundingMode": _VARIANT,
    "Stage": _VARIANT,
    "StubKind": _VARIANT,
    "Tenor": "a tenor such as '3M'",
    "TenorMatchMode": _VARIANT,
    "TenorUnit": _VARIANT,
    "TimeRollMode": _VARIANT,
    "TimeSeriesOp": _VARIANT,
    "VolHorizon": "the label 'one_step' or 'unconditional'",
}

# Parameters whose subject is the JSON text: (stub path under the package, qualified name, parameter).
TEXT_PARAMETERS: dict[tuple[str, str, str], str] = {
    ("portfolio/_cashflows.pyi", "net_in_currency_by_date", "cashflows"): (
        "also reads a by-date ladder that PortfolioCashflows.from_json does not load"
    ),
    ("scenarios/_templates.pyi", "validate_scenario_spec", "scenario"): (
        "reports on a spec that ScenarioSpec.from_json rejects"
    ),
}

_VALIDATES = "validates JSON text; the text is its subject"
_NO_TYPED_MEMBER = "no typed member holds this field"

# Public functions and members named `*_json`: (stub path under the package, qualified name).
JSON_NAMED: dict[tuple[str, str], str] = {
    ("attribution/_entry.pyi", "validate_attribution_json"): _VALIDATES,
    ("attribution/_entry.pyi", "validate_return_contribution_json"): _VALIDATES,
    ("cashflows/_schedule_json.pyi", "validate_cashflow_schedule_json"): _VALIDATES,
    ("covenants/_operations.pyi", "validate_covenant_engine_json"): _VALIDATES,
    ("covenants/_operations.pyi", "validate_covenant_report_json"): _VALIDATES,
    ("covenants/_operations.pyi", "validate_covenant_spec_json"): _VALIDATES,
    ("portfolio/_cashflows.pyi", "PortfolioCashflows.collapse_to_base_by_date_kind_json"): (
        "exact decimal amounts; the typed form is a float64 table"
    ),
    ("portfolio/_container.pyi", "parse_portfolio_spec_json"): _VALIDATES,
    ("portfolio/_factor_model/assignment.pyi", "PositionAssignment.mappings_json"): _NO_TYPED_MEMBER,
    ("portfolio/_factor_model/assignment.pyi", "UnmatchedEntry.dependency_json"): _NO_TYPED_MEMBER,
    ("portfolio/_optimization/spec_result.pyi", "PortfolioOptimizationSpec.portfolio_spec_json"): _NO_TYPED_MEMBER,
    ("valuations/instruments/_json_validation.pyi", "pretty_instrument_json"): (
        "re-renders instrument JSON text for reading; the text is its subject"
    ),
    ("valuations/instruments/_json_validation.pyi", "validate_instrument_json"): _VALIDATES,
    ("valuations/instruments/_json_validation.pyi", "validate_typed_instrument_json"): _VALIDATES,
    ("valuations/schema.pyi", "validate_instrument_envelope_json"): _VALIDATES,
    ("valuations/schema.pyi", "validate_instrument_type_json"): _VALIDATES,
}

# The two names that carry JSON text for every type.
_JSON_FORM_NAMES = frozenset({"to_json", "from_json"})


@dataclass(frozen=True)
class Finding:
    """One parameter that takes a typed object or the same object as text."""

    path: Path
    line: int
    owner: str
    parameter: str
    classes: tuple[str, ...]

    def format(self, root: Path) -> str:
        """Render the finding in compiler-style form.

        Parameters
        ----------
        root : Path
            Directory the reported path is shown relative to.

        Returns:
        -------
        str
            ``path:line: message`` for one finding.
        """
        try:
            shown = self.path.relative_to(root)
        except ValueError:
            shown = self.path
        named = ", ".join(self.classes[:3]) + (", ..." if len(self.classes) > 3 else "")
        return (
            f"{shown}:{self.line}: {self.owner}({self.parameter}) takes `str` beside {named}; "
            "a str is a value, so drop `str` and load JSON text with the class's loader or `json.loads`"
        )


def _name(node: ast.expr) -> str | None:
    """Return the last name of ``Name`` or ``module.Name``, else ``None``."""
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return node.attr
    return None


def type_aliases(tree: ast.Module) -> dict[str, ast.expr]:
    """Collect the module-level type aliases of one stub.

    Parameters
    ----------
    tree : ast.Module
        Parsed stub module.

    Returns:
    -------
    dict[str, ast.expr]
        Alias name to the annotation it stands for (``X = A | B``,
        ``X: TypeAlias = A | B`` and ``type X = A | B``).
    """
    aliases: dict[str, ast.expr] = {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            aliases[node.targets[0].id] = node.value
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name) and node.value is not None:
            aliases[node.target.id] = node.value
        elif isinstance(node, ast.TypeAlias):
            aliases[node.name.id] = node.value
    return aliases


def _union_members(annotation: ast.expr, aliases: dict[str, ast.expr], seen: frozenset[str]) -> list[ast.expr] | None:
    """Return the members of ``annotation`` if it is a union, expanding aliases; else ``None``."""
    if isinstance(annotation, ast.BinOp) and isinstance(annotation.op, ast.BitOr):
        sides = [annotation.left, annotation.right]
    elif isinstance(annotation, ast.Subscript) and _name(annotation.value) in {"Union", "Optional"}:
        sides = list(annotation.slice.elts) if isinstance(annotation.slice, ast.Tuple) else [annotation.slice]
    else:
        return None
    members: list[ast.expr] = []
    for side in sides:
        members.extend(_flatten(side, aliases, seen))
    return members


def _flatten(node: ast.expr, aliases: dict[str, ast.expr], seen: frozenset[str]) -> list[ast.expr]:
    """Return ``node`` as union members: itself, or the members it stands for."""
    if isinstance(node, ast.Constant) and isinstance(node.value, str):
        try:
            node = ast.parse(node.value, mode="eval").body
        except SyntaxError:
            return [node]
    name = _name(node)
    if name in aliases and name not in seen:
        return _flatten(aliases[name], aliases, seen | {name})
    nested = _union_members(node, aliases, seen)
    return [node] if nested is None else nested


def _union_levels(annotation: ast.expr, aliases: dict[str, ast.expr]) -> Iterator[list[ast.expr]]:
    """Yield the members of every union inside an annotation, one list per union."""
    members = _flatten(annotation, aliases, frozenset())
    if len(members) > 1:
        yield members
    for member in members:
        if isinstance(member, ast.Subscript):
            elements = member.slice.elts if isinstance(member.slice, ast.Tuple) else [member.slice]
            for element in elements:
                yield from _union_levels(element, aliases)


def package_classes(trees: dict[Path, ast.Module]) -> set[str]:
    """Collect the names of the classes the stubs define.

    Parameters
    ----------
    trees : dict[Path, ast.Module]
        Parsed stub modules keyed by their path.

    Returns:
    -------
    set[str]
        Name of every class defined anywhere in the stubs.
    """
    return {node.name for tree in trees.values() for node in ast.walk(tree) if isinstance(node, ast.ClassDef)}


def _parameters(function: ast.FunctionDef) -> Iterator[ast.arg]:
    arguments = function.args
    yield from arguments.posonlyargs
    yield from arguments.args
    yield from arguments.kwonlyargs
    if arguments.vararg is not None:
        yield arguments.vararg
    if arguments.kwarg is not None:
        yield arguments.kwarg


def _functions(body: Sequence[ast.stmt], owner: tuple[str, ...]) -> Iterator[tuple[str, ast.FunctionDef]]:
    for node in body:
        if isinstance(node, ast.ClassDef):
            yield from _functions(node.body, (*owner, node.name))
        elif isinstance(node, ast.FunctionDef):
            yield ".".join((*owner, node.name)), node


def _declared_names(body: Sequence[ast.stmt], owner: tuple[str, ...]) -> Iterator[tuple[str, int]]:
    """Yield ``(qualified name, line)`` for each function and annotated attribute, classes included."""
    for node in body:
        if isinstance(node, ast.ClassDef):
            yield from _declared_names(node.body, (*owner, node.name))
        elif isinstance(node, ast.FunctionDef):
            yield ".".join((*owner, node.name)), node.lineno
        elif isinstance(node, ast.AnnAssign) and isinstance(node.target, ast.Name):
            yield ".".join((*owner, node.target.id)), node.lineno


def find_typed_or_text(trees: dict[Path, ast.Module], classes: set[str]) -> list[Finding]:
    """List every parameter annotated with ``str`` beside a package class.

    Parameters
    ----------
    trees : dict[Path, ast.Module]
        Parsed stub modules keyed by their path.
    classes : set[str]
        Names of the classes the package defines.

    Returns:
    -------
    list[Finding]
        One finding per parameter, before any exemption is applied.
    """
    shared: dict[str, ast.expr] = {}
    for tree in trees.values():
        shared.update(type_aliases(tree))
    findings: list[Finding] = []
    for path, tree in trees.items():
        aliases = {**shared, **type_aliases(tree)}
        for owner, function in _functions(tree.body, ()):
            for parameter in _parameters(function):
                if parameter.annotation is None:
                    continue
                beside: set[str] = set()
                for members in _union_levels(parameter.annotation, aliases):
                    names = {_name(member) for member in members}
                    if "str" in names:
                        beside.update(name for name in names if name in classes)
                if beside:
                    findings.append(Finding(path, parameter.lineno, owner, parameter.arg, tuple(sorted(beside))))
    return findings


def find_json_named(trees: dict[Path, ast.Module]) -> list[tuple[Path, int, str]]:
    """List every public function or member whose name ends in ``_json``.

    Parameters
    ----------
    trees : dict[Path, ast.Module]
        Parsed stub modules keyed by their path.

    Returns:
    -------
    list[tuple[Path, int, str]]
        ``(path, line, qualified name)`` for each function, method, property
        and annotated attribute, before any exemption is applied.
        ``to_json``, ``from_json``, names with a leading underscore and
        members of a class with a leading underscore are not listed.
    """
    named: list[tuple[Path, int, str]] = []
    for path, tree in trees.items():
        for owner, line in _declared_names(tree.body, ()):
            parts = owner.split(".")
            if parts[-1] in _JSON_FORM_NAMES or not parts[-1].endswith("_json"):
                continue
            if any(part.startswith("_") for part in parts):
                continue
            named.append((path, line, owner))
    return named


def check(stub_root: Path) -> list[str]:
    """Check every stub under ``stub_root`` and return the diagnostics.

    Parameters
    ----------
    stub_root : Path
        Package directory whose ``.pyi`` files are read.

    Returns:
    -------
    list[str]
        One line per violation or stale exemption; empty when the tree is clean.
    """
    trees = {path: ast.parse(path.read_text(encoding="utf-8")) for path in sorted(stub_root.rglob("*.pyi"))}
    used_labels: set[str] = set()
    used_text: set[tuple[str, str, str]] = set()
    diagnostics: list[str] = []
    for finding in find_typed_or_text(trees, package_classes(trees)):
        key = (finding.path.relative_to(stub_root).as_posix(), finding.owner, finding.parameter)
        if key in TEXT_PARAMETERS:
            used_text.add(key)
            continue
        used_labels.update(name for name in finding.classes if name in LABEL_CLASSES)
        documents = tuple(name for name in finding.classes if name not in LABEL_CLASSES)
        if documents:
            diagnostics.append(
                Finding(finding.path, finding.line, finding.owner, finding.parameter, documents).format(
                    stub_root.parent.parent
                )
            )
    used_named: set[tuple[str, str]] = set()
    for path, line, owner in find_json_named(trees):
        key = (path.relative_to(stub_root).as_posix(), owner)
        if key in JSON_NAMED:
            used_named.add(key)
            continue
        try:
            shown = path.relative_to(stub_root.parent.parent)
        except ValueError:
            shown = path
        diagnostics.append(
            f"{shown}:{line}: {owner} is named `*_json`; return the typed value and let callers take "
            "text from `to_json()`, or list it in JSON_NAMED with its reason"
        )
    diagnostics.extend(
        f"LABEL_CLASSES entry {name!r} matches no parameter; remove it"
        for name in sorted(set(LABEL_CLASSES) - used_labels)
    )
    diagnostics.extend(
        f"TEXT_PARAMETERS entry {key!r} matches no parameter; remove it"
        for key in sorted(set(TEXT_PARAMETERS) - used_text)
    )
    diagnostics.extend(
        f"JSON_NAMED entry {key!r} matches no function; remove it" for key in sorted(set(JSON_NAMED) - used_named)
    )
    return diagnostics


def main(argv: Sequence[str] | None = None) -> int:
    """Run the check from the command line.

    Parameters
    ----------
    argv : Sequence[str] or None
        Command-line arguments without the program name; ``None`` reads ``sys.argv``.

    Returns:
    -------
    int
        ``0`` when the stubs are clean, ``1`` otherwise.
    """
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--stub-root", type=Path, default=STUB_ROOT, help="package directory holding the .pyi stubs")
    arguments = parser.parse_args(argv)
    diagnostics = check(arguments.stub_root)
    for line in diagnostics:
        print(line, file=sys.stderr)
    if diagnostics:
        print(f"Python input forms: {len(diagnostics)} problem(s)", file=sys.stderr)
        return 1
    count = sum(1 for _ in arguments.stub_root.rglob("*.pyi"))
    print(f"Python input forms: clean ({count} stub files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
