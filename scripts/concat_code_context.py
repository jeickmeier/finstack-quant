"""Concatenate selected code/text files into a Markdown context bundle."""

from __future__ import annotations

import argparse
from collections.abc import Iterable, Sequence
from pathlib import Path
import sys

DEFAULT_EXTENSIONS = frozenset({
    ".c",
    ".cfg",
    ".cjs",
    ".cpp",
    ".css",
    ".csv",
    ".h",
    ".hpp",
    ".html",
    ".ini",
    ".java",
    ".js",
    ".json",
    ".jsx",
    ".lock",
    ".md",
    ".mjs",
    ".py",
    ".rs",
    ".sh",
    ".sql",
    ".toml",
    ".ts",
    ".tsx",
    ".txt",
    ".xml",
    ".yaml",
    ".yml",
})

DEFAULT_EXCLUDED_NAMES = frozenset({
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".ty_cache",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
})

DEFAULT_OUTPUT_ROOT = Path("code-context-output")

LANGUAGE_BY_EXTENSION = {
    ".c": "c",
    ".cjs": "javascript",
    ".cpp": "cpp",
    ".css": "css",
    ".h": "c",
    ".hpp": "cpp",
    ".html": "html",
    ".java": "java",
    ".js": "javascript",
    ".json": "json",
    ".jsx": "jsx",
    ".md": "markdown",
    ".mjs": "javascript",
    ".py": "python",
    ".rs": "rust",
    ".sh": "bash",
    ".sql": "sql",
    ".toml": "toml",
    ".ts": "typescript",
    ".tsx": "tsx",
    ".xml": "xml",
    ".yaml": "yaml",
    ".yml": "yaml",
}


def build_markdown(
    inputs: Sequence[Path],
    *,
    root: Path | None = None,
    extensions: Iterable[str] = DEFAULT_EXTENSIONS,
    excluded_names: Iterable[str] = DEFAULT_EXCLUDED_NAMES,
    max_file_bytes: int = 1_000_000,
) -> tuple[str, list[str]]:
    """Build a Markdown bundle from selected input paths."""
    bundle_root = (root or Path.cwd()).resolve()
    normalized_extensions = {normalize_extension(extension) for extension in extensions}
    excluded = set(excluded_names)
    warnings: list[str] = []
    files = collect_files(inputs, bundle_root, normalized_extensions, excluded, warnings)

    lines = [
        "# Code Context Bundle",
        "",
        "This is a code/text extract intended for LLM context.",
        "It includes only selected common code/text extensions and skips binaries, generated artifacts, caches, and other noisy outputs.",
        "It may omit relevant docs, runtime state, dependency metadata, or files excluded by the filters below.",
        "",
        "## Requested Sources",
        "",
    ]

    for input_path in inputs:
        lines.append(f"- `{display_path(input_path, bundle_root)}`")

    lines.extend([
        "",
        "## Included Filters",
        "",
        f"- Extensions: {', '.join(sorted(normalized_extensions))}",
        f"- Excluded names: {', '.join(sorted(excluded))}",
        f"- Max file size: {max_file_bytes} bytes",
        "",
        "## Files",
        "",
    ])

    for file_path in files:
        relative = display_path(file_path, bundle_root)
        try:
            if file_path.stat().st_size > max_file_bytes:
                warnings.append(f"Skipped {relative}: file is larger than {max_file_bytes} bytes")
                continue
            content = file_path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            warnings.append(f"Skipped {relative}: not valid UTF-8 text")
            continue
        except OSError as exc:
            warnings.append(f"Skipped {relative}: {exc}")
            continue

        language = LANGUAGE_BY_EXTENSION.get(file_path.suffix.lower(), "")
        lines.extend([f"## `{relative}`", "", f"```{language}", content.rstrip(), "```", ""])

    return "\n".join(lines).rstrip() + "\n", warnings


def collect_files(
    inputs: Sequence[Path],
    root: Path,
    extensions: set[str],
    excluded_names: set[str],
    warnings: list[str],
) -> list[Path]:
    """Collect matching files in deterministic order."""
    files: list[Path] = []
    seen: set[Path] = set()

    for input_path in inputs:
        path = input_path if input_path.is_absolute() else root / input_path
        resolved = path.resolve()

        if not resolved.exists():
            warnings.append(f"Skipped {display_path(resolved, root)}: path does not exist")
            continue

        candidates = [resolved] if resolved.is_file() else resolved.rglob("*")
        for candidate in candidates:
            if should_skip(candidate, excluded_names):
                continue
            if not candidate.is_file() or candidate.suffix.lower() not in extensions:
                continue
            if candidate not in seen:
                files.append(candidate)
                seen.add(candidate)

    return sorted(files, key=lambda path: display_path(path, root))


def should_skip(path: Path, excluded_names: set[str]) -> bool:
    """Return true when any path component is excluded."""
    return any(part in excluded_names for part in path.parts)


def normalize_extension(extension: str) -> str:
    """Normalize CLI extension values to dot-prefixed lowercase extensions."""
    stripped = extension.strip().lower()
    if not stripped:
        msg = "Extension values must not be empty"
        raise ValueError(msg)
    return stripped if stripped.startswith(".") else f".{stripped}"


def display_path(path: Path, root: Path) -> str:
    """Format paths relative to the bundle root when possible."""
    resolved = path.resolve()
    try:
        return resolved.relative_to(root).as_posix()
    except ValueError:
        return resolved.as_posix()


def parse_args(argv: Sequence[str]) -> argparse.Namespace:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Concatenate code/text files from selected paths into one Markdown file.",
    )
    parser.add_argument("paths", nargs="+", type=Path, help="Files or directories to include")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path("code-context.md"),
        help="Markdown output filename or path under --output-root (default: code-context.md)",
    )
    parser.add_argument(
        "--output-root",
        type=Path,
        default=DEFAULT_OUTPUT_ROOT,
        help=f"Root directory for relative output paths (default: {DEFAULT_OUTPUT_ROOT})",
    )
    parser.add_argument(
        "--include-ext",
        action="append",
        default=[],
        help="Additional extension to include, e.g. --include-ext rst. May be repeated.",
    )
    parser.add_argument(
        "--exclude",
        action="append",
        default=[],
        help="Additional file or directory name to exclude. May be repeated.",
    )
    parser.add_argument(
        "--max-file-bytes",
        type=int,
        default=1_000_000,
        help="Skip files larger than this many bytes (default: 1000000)",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    """Run the command-line utility."""
    args = parse_args(argv or sys.argv[1:])
    if args.max_file_bytes < 1:
        print("--max-file-bytes must be greater than zero", file=sys.stderr)
        return 2

    try:
        extensions = set(DEFAULT_EXTENSIONS) | {normalize_extension(extension) for extension in args.include_ext}
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2

    output_path = args.output if args.output.is_absolute() else args.output_root / args.output
    excluded_names = set(DEFAULT_EXCLUDED_NAMES) | {args.output_root.name} | set(args.exclude)
    markdown, warnings = build_markdown(
        args.paths,
        extensions=extensions,
        excluded_names=excluded_names,
        max_file_bytes=args.max_file_bytes,
    )

    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        output_path.write_text(markdown, encoding="utf-8")
    except OSError as exc:
        print(f"Failed to write {output_path}: {exc}", file=sys.stderr)
        return 1

    for warning in warnings:
        print(f"warning: {warning}", file=sys.stderr)

    print(f"Wrote {output_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
