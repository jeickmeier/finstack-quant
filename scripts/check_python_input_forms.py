"""Reject a Python parameter that takes a typed object or the same object as text.

In the Python API a ``str`` argument is a value: a label, a code, a date, an id.
It is never a JSON document. A class that can be loaded from JSON text has a
named loader (``from_json`` or ``from_spec``), so a parameter annotated
``Class | str`` offers two ways to pass one value.

This check reads the ``.pyi`` stubs under ``finstack-quant-py/finstack_quant``.
It fails on any parameter whose annotation holds ``str`` in the same union as a
class that defines such a loader. Two short lists are exempt, each entry with
its reason: classes for which a string is a label, and the parameters whose
subject is the JSON text itself. An entry that no longer matches anything is
also an error, so the lists cannot go stale.

The check sees annotations only. ``finstack-quant-py/tests/test_json_text_inputs.py``
proves the runtime behaviour.
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

LOADER_METHODS = frozenset({"from_json", "from_spec"})

# Classes whose string form is a label or a number in text, not a document.
LABEL_CLASSES: dict[str, str] = {
    "Bps": "a basis-point amount in text",
    "CreditRating": "a rating label such as 'BBB+'",
    "Currency": "an ISO 4217 code",
    "FeeBase": "the label of a fee base without parameters",
    "Money": "a decimal amount in text",
    "Rate": "a decimal rate in text",
    "RoundingMode": "a rounding-mode name",
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


@dataclass(frozen=True)
class Finding:
    """One parameter that takes a typed object or the same object as text."""

    path: Path
    line: int
    owner: str
    parameter: str
    class_name: str

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
        return (
            f"{shown}:{self.line}: {self.owner}({self.parameter}) takes `{self.class_name} | str`; "
            f"a str is a value, so drop `str` and load JSON text with `{self.class_name}.from_json`"
        )


def _union_levels(annotation: ast.expr) -> Iterator[list[ast.expr]]:
    """Yield the members of every union inside an annotation, one list per union."""
    if isinstance(annotation, ast.BinOp) and isinstance(annotation.op, ast.BitOr):
        members: list[ast.expr] = []
        stack = [annotation]
        while stack:
            node = stack.pop()
            if isinstance(node, ast.BinOp) and isinstance(node.op, ast.BitOr):
                stack.extend((node.right, node.left))
            else:
                members.append(node)
        yield members
        for member in members:
            yield from _union_levels(member)
    elif isinstance(annotation, ast.Subscript):
        elements = annotation.slice.elts if isinstance(annotation.slice, ast.Tuple) else [annotation.slice]
        if _name(annotation.value) in {"Union", "Optional"}:
            yield list(elements)
        for element in elements:
            yield from _union_levels(element)
    elif isinstance(annotation, ast.Constant) and isinstance(annotation.value, str):
        try:
            quoted = ast.parse(annotation.value, mode="eval").body
        except SyntaxError:
            return
        yield from _union_levels(quoted)


def _name(node: ast.expr) -> str | None:
    """Return the last name of ``Name`` or ``module.Name``, else ``None``."""
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        return node.attr
    return None


def loader_classes(trees: dict[Path, ast.Module]) -> set[str]:
    """Collect the names of the classes that define a JSON loader.

    Parameters
    ----------
    trees : dict[Path, ast.Module]
        Parsed stub modules keyed by their path.

    Returns:
    -------
    set[str]
        Names of the classes with a ``from_json`` or ``from_spec`` method.
    """
    names: set[str] = set()
    for tree in trees.values():
        for node in ast.walk(tree):
            if isinstance(node, ast.ClassDef) and any(
                isinstance(item, ast.FunctionDef) and item.name in LOADER_METHODS for item in node.body
            ):
                names.add(node.name)
    return names


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


def find_typed_or_text(trees: dict[Path, ast.Module], loaders: set[str]) -> list[Finding]:
    """List every parameter annotated with ``str`` beside a loadable class.

    Parameters
    ----------
    trees : dict[Path, ast.Module]
        Parsed stub modules keyed by their path.
    loaders : set[str]
        Names of the classes that define a JSON loader.

    Returns:
    -------
    list[Finding]
        One finding per parameter and class, before any exemption is applied.
    """
    findings: list[Finding] = []
    for path, tree in trees.items():
        for owner, function in _functions(tree.body, ()):
            for parameter in _parameters(function):
                if parameter.annotation is None:
                    continue
                classes: set[str] = set()
                for members in _union_levels(parameter.annotation):
                    names = {_name(member) for member in members}
                    if "str" in names:
                        classes.update(name for name in names if name in loaders)
                findings.extend(
                    Finding(path, parameter.lineno, owner, parameter.arg, class_name) for class_name in sorted(classes)
                )
    return findings


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
    findings = find_typed_or_text(trees, loader_classes(trees))
    used_labels: set[str] = set()
    used_text: set[tuple[str, str, str]] = set()
    diagnostics: list[str] = []
    for finding in findings:
        key = (finding.path.relative_to(stub_root).as_posix(), finding.owner, finding.parameter)
        if key in TEXT_PARAMETERS:
            used_text.add(key)
        elif finding.class_name in LABEL_CLASSES:
            used_labels.add(finding.class_name)
        else:
            diagnostics.append(finding.format(stub_root.parent.parent))
    diagnostics.extend(
        f"LABEL_CLASSES entry {name!r} matches no parameter; remove it"
        for name in sorted(set(LABEL_CLASSES) - used_labels)
    )
    diagnostics.extend(
        f"TEXT_PARAMETERS entry {key!r} matches no parameter; remove it"
        for key in sorted(set(TEXT_PARAMETERS) - used_text)
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
