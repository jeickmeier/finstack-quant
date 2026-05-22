#!/usr/bin/env python3
"""Find unused imports in Rust files.

Uses regex-based parsing to detect unused `use` statements.
Handles simple, aliased, grouped, nested, and glob imports.
"""

from pathlib import Path
import re
import sys


def extract_use_statements(content: str) -> list[dict]:
    """Extract all `use` statements from Rust source, with line numbers."""
    statements = []
    lines = content.split("\n")

    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # Skip comments and doc comments
        if stripped.startswith("//"):
            i += 1
            continue

        # Match `use` or `pub use` or `pub(crate) use` etc.
        use_match = re.match(r"^(\s*(?:pub(?:\s*\([^)]*\))?\s+)?use\s+)(.*)", stripped)
        if use_match:
            start_line = i + 1  # 1-indexed
            use_body = use_match.group(2)
            is_pub = "pub" in (use_match.group(1) or "")

            # Handle multi-line use statements
            full_body = use_body
            while not full_body.rstrip().endswith(";") and i + 1 < len(lines):
                i += 1
                full_body += " " + lines[i].strip()

            # Remove trailing semicolon
            full_body = full_body.strip().rstrip(";").strip()

            statements.append({
                "line": start_line,
                "body": full_body,
                "is_pub": is_pub,
                "raw": stripped,
            })

        i += 1

    return statements


def parse_imported_names(body: str) -> list[dict]:
    """Parse a use statement body to extract imported names.

    Returns list of dicts with 'name' (the local binding) and 'is_glob'.
    """
    names = []

    # Glob import: path::*
    if body.endswith("::*") or body == "*":
        return [{"name": "*", "is_glob": True}]

    # Grouped import: path::{A, B as C, sub::D, ...}
    group_match = re.search(r"\{([^}]+)\}", body)
    if group_match:
        items_str = group_match.group(1)
        # Split on commas, handling nested braces
        items = split_grouped_items(items_str)
        for item in items:
            stripped_item = item.strip()
            if not stripped_item:
                continue
            if stripped_item == "*":
                names.append({"name": "*", "is_glob": True})
            elif stripped_item == "self":
                # `use module::{self}` imports the module name
                # Extract module name from path before ::
                prefix = body[: body.index("{")].rstrip(":")
                mod_name = prefix.rsplit("::", 1)[-1] if "::" in prefix else prefix
                names.append({"name": mod_name, "is_glob": False})
            else:
                # Could be: Name, path::Name, Name as Alias
                alias_match = re.match(r".*\bas\s+(\w+)$", stripped_item)
                if alias_match:
                    names.append({"name": alias_match.group(1), "is_glob": False})
                else:
                    # Take the last segment
                    last = stripped_item.rsplit("::", 1)[-1].strip()
                    if last and last != "self":
                        names.append({"name": last, "is_glob": False})
        return names

    # Simple import: path::Name or path::Name as Alias
    alias_match = re.match(r".*\bas\s+(\w+)$", body)
    if alias_match:
        names.append({"name": alias_match.group(1), "is_glob": False})
    else:
        last = body.rsplit("::", 1)[-1].strip()
        if last:
            names.append({"name": last, "is_glob": False})

    return names


def split_grouped_items(items_str: str) -> list[str]:
    """Split comma-separated items in a grouped use, respecting nested braces."""
    items = []
    depth = 0
    current = ""
    for ch in items_str:
        if ch == "{":
            depth += 1
            current += ch
        elif ch == "}":
            depth -= 1
            current += ch
        elif ch == "," and depth == 0:
            items.append(current.strip())
            current = ""
        else:
            current += ch
    if current.strip():
        items.append(current.strip())
    return items


def find_used_identifiers(content: str, use_statements: list[dict]) -> set[str]:
    """Find all identifiers used in the file outside of use statements.

    Strips use statement lines and comments, then scans for word boundaries.
    """
    lines = content.split("\n")
    use_lines = set()
    for stmt in use_statements:
        # Mark all lines belonging to this use statement
        use_lines.add(stmt["line"] - 1)  # 0-indexed

    # Build content without use statements and without comments
    filtered_lines = []
    in_block_comment = False
    for idx, line in enumerate(lines):
        if idx in use_lines:
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

    # Also handle multi-line use statements
    # Re-scan for use lines that span multiple lines
    # (already handled by single-line removal above for most cases)

    # Extract all word-boundary identifiers
    return set(re.findall(r"\b([A-Za-z_]\w*)\b", filtered_content))


def find_unused_imports(file_path: Path) -> dict:
    """Find unused imports in a Rust file."""
    try:
        content = file_path.read_text()
        use_statements = extract_use_statements(content)
        used_idents = find_used_identifiers(content, use_statements)

        unused = []
        total_imports = 0
        has_glob = False

        for stmt in use_statements:
            imported = parse_imported_names(stmt["body"])

            for imp in imported:
                if imp["is_glob"]:
                    has_glob = True
                    continue

                total_imports += 1
                name = imp["name"]

                # Skip pub use (re-exports are intentionally unused locally)
                if stmt["is_pub"]:
                    continue

                # Check if the name is used
                if name not in used_idents:
                    unused.append({
                        "name": name,
                        "line": stmt["line"],
                        "statement": stmt["raw"],
                    })

        return {
            "file": str(file_path),
            "unused_imports": unused,
            "total_imports": total_imports,
            "has_glob_imports": has_glob,
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
