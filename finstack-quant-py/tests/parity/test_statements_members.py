"""Every public statements class member has a matching published declaration."""

import ast
from pathlib import Path

from finstack_quant import statements


def test_statements_class_members_match_published_stub() -> None:
    stub = Path(statements.__file__).with_suffix(".pyi")
    tree = ast.parse(stub.read_text(encoding="utf-8"))
    differences = []
    for node in tree.body:
        if not isinstance(node, ast.ClassDef):
            continue
        runtime_class = getattr(statements, node.name)
        declared = {
            item.name
            for item in node.body
            if isinstance(item, ast.FunctionDef) and (not item.name.startswith("_") or item.name == "__init__")
        }
        runtime = {name for name in dir(runtime_class) if not name.startswith("_")}
        if runtime_class.__text_signature__ is not None:
            runtime.add("__init__")
        if declared != runtime:
            differences.append(
                f"{node.name}: missing declarations={sorted(runtime - declared)}, "
                f"missing runtime members={sorted(declared - runtime)}"
            )
    assert not differences, "\n".join(differences)
