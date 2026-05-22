#!/usr/bin/env python3
"""Find unused imports in TSX/TS/JSX/JS files.

Uses regex-based parsing to detect unused import statements.
Handles default, named, namespace, type-only, and side-effect imports.
"""

from pathlib import Path
import re
import sys


def extract_import_statements(content: str) -> list[dict]:
    """Extract all import statements from TS/TSX source, with line numbers."""
    statements = []
    lines = content.split("\n")

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # Skip comments
        if stripped.startswith("//"):
            i += 1
            continue

        # Match import statements
        # Patterns:
        #   import ... from '...';
        #   import '...';  (side-effect)
        #   import type ... from '...';
        if re.match(r"^import\s", stripped):
            start_line = i + 1  # 1-indexed
            full_line = stripped

            # Handle multi-line imports
            while not _has_complete_import(full_line) and i + 1 < len(lines):
                i += 1
                full_line += " " + lines[i].strip()

            statements.append({
                "line": start_line,
                "raw": full_line,
            })

        i += 1

    return statements


def _has_complete_import(text: str) -> bool:
    """Check if an import statement is complete (has closing quote/semicolon)."""
    # A complete import has a from clause with quotes, or is a side-effect import
    # Check for: from '...' or from "..." or just '...' or "..."
    if re.search(r"""from\s+['"][^'"]*['"]""", text):
        return True
    if re.search(r"""^import\s+['"][^'"]*['"]""", text):
        return True
    # Also complete if ends with semicolon and has quotes
    return bool(text.rstrip().endswith(";") and ("'" in text or '"' in text))


def parse_imported_names(raw: str) -> list[dict]:
    """Parse an import statement to extract imported names.

    Returns list of dicts with:
      - 'name': the local binding name
      - 'is_type': whether it's a type-only import
      - 'is_side_effect': whether it's a side-effect-only import
      - 'is_namespace': whether it's a namespace import (* as name)
    """
    names = []

    # Remove trailing semicolon
    stmt = raw.rstrip(";").strip()

    # Side-effect import: import 'module' or import "module"
    side_effect_match = re.match(r"""^import\s+['"].*['"]$""", stmt)
    if side_effect_match:
        return [{"name": None, "is_type": False, "is_side_effect": True, "is_namespace": False}]

    # Check for type-only import
    is_type_import = bool(re.match(r"^import\s+type\s", stmt))

    # Remove `import type` or `import` prefix
    body = re.sub(r"^import\s+type\s+", "", stmt)
    body = re.sub(r"^import\s+", "", body)

    # Remove the `from '...'` / `from "..."` suffix
    body = re.sub(r"""\s+from\s+['"].*['"]$""", "", body)

    body = body.strip()
    if not body:
        return names

    # Split default from named/namespace: `default, { ... }` or `default, * as ns`
    parts = _split_default_and_rest(body)

    for part in parts:
        stripped_part = part.strip()
        if not stripped_part:
            continue

        # Namespace import: * as name
        ns_match = re.match(r"^\*\s+as\s+(\w+)$", stripped_part)
        if ns_match:
            names.append({
                "name": ns_match.group(1),
                "is_type": is_type_import,
                "is_side_effect": False,
                "is_namespace": True,
            })
            continue

        # Named imports: { A, B as C, type D, ... }
        named_match = re.match(r"^\{(.*)\}$", stripped_part, re.DOTALL)
        if named_match:
            items_str = named_match.group(1)
            items = [x.strip() for x in items_str.split(",")]
            for item in items:
                if not item:
                    continue
                # Inline type: `type Foo` or `type Foo as Bar`
                inline_type = False
                stripped_item = item
                if re.match(r"^type\s+", stripped_item):
                    inline_type = True
                    stripped_item = re.sub(r"^type\s+", "", stripped_item)

                alias_match = re.match(r"(\w+)\s+as\s+(\w+)", stripped_item)
                local_name = alias_match.group(2) if alias_match else stripped_item.strip()

                if local_name and re.match(r"^\w+$", local_name):
                    names.append({
                        "name": local_name,
                        "is_type": is_type_import or inline_type,
                        "is_side_effect": False,
                        "is_namespace": False,
                    })
            continue

        # Default import (plain identifier)
        if re.match(r"^\w+$", stripped_part):
            names.append({
                "name": stripped_part,
                "is_type": is_type_import,
                "is_side_effect": False,
                "is_namespace": False,
            })

    return names


def _split_default_and_rest(body: str) -> list[str]:
    """Split `Default, { named }` or `Default, * as ns` into parts."""
    parts = []
    depth = 0
    current = ""

    for ch in body:
        if ch == "{":
            depth += 1
            current += ch
        elif ch == "}":
            depth -= 1
            current += ch
        elif ch == "," and depth == 0:
            parts.append(current.strip())
            current = ""
        else:
            current += ch

    if current.strip():
        parts.append(current.strip())

    return parts


def find_used_identifiers(content: str, import_statements: list[dict]) -> set[str]:
    """Find all identifiers used in the file outside of import statements."""
    lines = content.split("\n")
    import_lines = set()
    for stmt in import_statements:
        import_lines.add(stmt["line"] - 1)  # 0-indexed

    # Build content without import lines and without comments
    filtered_lines = []
    in_block_comment = False

    for idx, line in enumerate(lines):
        if idx in import_lines:
            continue

        processed = ""
        i = 0
        while i < len(line):
            if in_block_comment:
                end = line.find("*/", i)
                if end == -1:
                    break
                in_block_comment = False
                i = end + 2
            elif line[i : i + 2] == "//":
                break
            elif line[i : i + 2] == "/*":
                in_block_comment = True
                i += 2
            else:
                processed += line[i]
                i += 1

        filtered_lines.append(processed)

    filtered_content = "\n".join(filtered_lines)

    # Also strip string literals (rough: single and double quoted, template literals)
    # This prevents false positives from names appearing in strings
    filtered_content = re.sub(r"`[^`]*`", '""', filtered_content)
    filtered_content = re.sub(r'"(?:[^"\\]|\\.)*"', '""', filtered_content)
    filtered_content = re.sub(r"'(?:[^'\\]|\\.)*'", "''", filtered_content)

    return set(re.findall(r"\b([A-Za-z_$]\w*)\b", filtered_content))


def find_unused_imports(file_path: Path) -> dict:
    """Find unused imports in a TSX/TS/JSX/JS file."""
    try:
        content = file_path.read_text()
        import_statements = extract_import_statements(content)
        used_idents = find_used_identifiers(content, import_statements)

        unused = []
        total_imports = 0
        side_effect_count = 0

        for stmt in import_statements:
            imported = parse_imported_names(stmt["raw"])

            for imp in imported:
                if imp["is_side_effect"]:
                    side_effect_count += 1
                    continue

                if imp["name"] is None:
                    continue

                total_imports += 1

                # Check if the name is used in the rest of the file
                if imp["name"] not in used_idents:
                    unused.append({
                        "name": imp["name"],
                        "line": stmt["line"],
                        "is_type": imp["is_type"],
                        "statement": stmt["raw"].strip(),
                    })

        return {
            "file": str(file_path),
            "unused_imports": unused,
            "total_imports": total_imports,
            "side_effect_imports": side_effect_count,
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
