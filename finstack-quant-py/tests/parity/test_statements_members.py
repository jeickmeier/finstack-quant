"""Every public statements class member has a matching published declaration."""

import ast

from tests.stub_helpers import stub_definitions

from finstack_quant import statements


def test_statements_class_members_match_published_stub() -> None:
    definitions = stub_definitions("finstack_quant.statements")
    assert any(isinstance(node, ast.ClassDef) for node in definitions)
    differences = []
    for node in definitions:
        if not isinstance(node, ast.ClassDef):
            continue
        runtime_class = getattr(statements, node.name)
        members: dict[str, list[ast.FunctionDef]] = {}
        for item in node.body:
            if isinstance(item, ast.FunctionDef):
                members.setdefault(item.name, []).append(item)
        for name, definitions in members.items():
            # Overload signatures and property setters legitimately share a name.
            implementations = [
                definition
                for definition in definitions
                if not any(
                    (isinstance(decorator, ast.Name) and decorator.id == "overload")
                    or (isinstance(decorator, ast.Attribute) and decorator.attr in {"setter", "deleter"})
                    for decorator in definition.decorator_list
                )
            ]
            if len(implementations) > 1:
                differences.append(f"{node.name}.{name}: duplicate declarations")
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
