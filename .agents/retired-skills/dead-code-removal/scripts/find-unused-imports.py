#!/usr/bin/env python3
"""Find unused imports in Python files.

Uses AST parsing to accurately detect unused imports.
"""

import ast
from pathlib import Path
import sys


class ImportVisitor(ast.NodeVisitor):
    """AST visitor to collect imports and their usage."""

    def __init__(self) -> None:
        """Initialize import tracking state."""
        self.imports = {}  # name -> line_number
        self.used_names = set()
        self.import_star = False

    def visit_Import(self, node: ast.Import) -> None:
        """Record direct import bindings."""
        for alias in node.names:
            name = alias.asname if alias.asname else alias.name.split(".")[0]
            self.imports[name] = node.lineno
        self.generic_visit(node)

    def visit_ImportFrom(self, node: ast.ImportFrom) -> None:
        """Record imported names from modules and note wildcard imports."""
        if node.module:
            if node.names[0].name == "*":
                self.import_star = True
            else:
                for alias in node.names:
                    name = alias.asname if alias.asname else alias.name
                    self.imports[name] = node.lineno
        self.generic_visit(node)

    def visit_Name(self, node: ast.Name) -> None:
        """Record loaded identifier usage."""
        if isinstance(node.ctx, ast.Load):
            self.used_names.add(node.id)
        self.generic_visit(node)

    def visit_Attribute(self, node: ast.Attribute) -> None:
        """Record attribute base names such as `os` in `os.path`."""
        # Handle cases like os.path, sys.argv
        if isinstance(node.value, ast.Name):
            self.used_names.add(node.value.id)
        self.generic_visit(node)


def find_unused_imports(file_path: Path) -> dict:
    """Find unused imports in a Python file."""
    try:
        content = file_path.read_text()
        tree = ast.parse(content, filename=str(file_path))

        visitor = ImportVisitor()
        visitor.visit(tree)

        # Find unused imports
        unused = []
        for name, line_num in visitor.imports.items():
            if name not in visitor.used_names and not visitor.import_star:
                unused.append({
                    "name": name,
                    "line": line_num,
                })

        return {
            "file": str(file_path),
            "unused_imports": unused,
            "total_imports": len(visitor.imports),
            "has_import_star": visitor.import_star,
        }
    except SyntaxError as e:
        return {
            "file": str(file_path),
            "error": f"Syntax error: {e}",
            "unused_imports": [],
        }
    except (OSError, UnicodeDecodeError) as e:
        return {
            "file": str(file_path),
            "error": f"Error parsing file: {e}",
            "unused_imports": [],
        }


def main() -> None:
    """Main entry point."""
    if len(sys.argv) < 2:
        sys.exit(1)

    results = []
    for file_path_str in sys.argv[1:]:
        file_path = Path(file_path_str)
        if not file_path.exists():
            continue

        result = find_unused_imports(file_path)
        results.append(result)


if __name__ == "__main__":
    main()
