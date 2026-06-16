#!/usr/bin/env python3
"""Audit hard-coded external assumptions that should move to registries.

The scanner is intentionally conservative: it flags source lines that contain
domain terms associated with market conventions, rating-agency studies,
regulatory policies, accounting policies, or product assumptions. Existing
JSON data registries are classified separately so migration progress can be
tracked without treating current registry files as leftovers.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import json
import os
from pathlib import Path
import re
import sys
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ALLOWLIST = Path("scripts/hardcoded_assumptions_allowlist.json")
DEFAULT_ROOTS = ("finstack-quant", "finstack-quant-py/src", "finstack-quant-wasm/src", "finstack-quant-wasm/exports")
SOURCE_EXTENSIONS = {".json", ".py", ".pyi", ".rs", ".toml", ".ts", ".tsx", ".js"}
EXCLUDED_DIRS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "__pycache__",
    "benchmarks",
    "benches",
    "examples",
    "node_modules",
    "pkg",
    "pkg-node",
    "schemas",
    "target",
    "target-codex",
    "tests",
}
EXCLUDED_PATH_PARTS = {
    "Cargo.lock",
    "package-lock.json",
    "uv.lock",
    "types/generated",
}
REGISTRY_CODE_PATH_PARTS = (
    "src/credit/registry.rs",
    "src/rating_scales.rs",
    "src/contract_specs.rs",
    "finstack-quant/analytics/src/registry.rs",
    "finstack-quant/valuations/src/calibration/defaults.rs",
    "finstack-quant/valuations/src/market/conventions/loaders/",
    "src/analysis/ecl/policy.rs",
    "finstack-quant/monte_carlo/src/registry.rs",
    "finstack-quant/margin/src/registry/",
    "finstack-quant/margin/src/regulatory/frtb/params/registry.rs",
    "structured_credit/assumptions.rs",
)

CATEGORY_PATTERNS: dict[str, tuple[str, ...]] = {
    "rating_agency": (
        r"\bmoody'?s\b",
        r"\bmoodys\b",
        r"\bs&p\b",
        r"\bstandard\s*(?:and|&)?\s*poor",
        r"\bfitch\b",
        r"\brating[_ -]?agency\b",
        r"\bwarf\b",
        r"\brating[_ -]?factor\b",
        r"\bmaster[_ -]?scale\b",
    ),
    "credit_assumption": (
        r"\brecovery(?:[_ -]?rate| rates?)?\b",
        r"\blgd\b",
        r"\bdefault[_ -]?rate\b",
        r"\bdefault[_ -]?factor\b",
        r"\bpd[_ -]?(?:master|scale|delta|relative|absolute)?\b",
        r"\bhazard\b",
        r"\bprobability[_ -]?of[_ -]?default\b",
    ),
    "structured_credit": (
        r"\bcdr\b",
        r"\bcpr\b",
        r"\bpsa\b",
        r"\bsda\b",
        r"\bseverity\b",
        r"\bseasonality\b",
        r"\bprepay(?:ment)?\b",
        r"\bburnout\b",
        r"\bservicing[_ -]?fee\b",
        r"\btrustee[_ -]?fee\b",
        r"\bmanagement[_ -]?fee\b",
        r"\bconcentration\b",
    ),
    "market_convention": (
        r"\bsettlement[_ -]?days?\b",
        r"\bpayment[_ -]?lag\b",
        r"\breset[_ -]?lag\b",
        r"\bobservation[_ -]?shift\b",
        r"\bcalendar[_ -]?id\b",
        r"\bbusiness[_ -]?day[_ -]?convention\b",
        r"\bdiscount[_ -]?curve\b",
        r"\bforward[_ -]?curve\b",
    ),
    "contract_spec": (
        r"\btick[_ -]?(?:size|value)\b",
        r"\bmultiplier\b",
        r"\bcontract[_ -]?size\b",
        r"\bface[_ -]?value\b",
        r"\bdelivery[_ -]?months\b",
        r"\bstandard[_ -]?coupon\b",
        r"\bindex[_ -]?id\b",
    ),
    "margin_regulatory": (
        r"\bmpor\b",
        r"\bhaircut\b",
        r"\bcollateral\b",
        r"\bsimm\b",
        r"\bccp\b",
        r"\bgeneric[_ -]?var\b",
        r"\bbasel\b",
        r"\bxva\b",
        r"\bcva\b",
        r"\bfva\b",
    ),
    "accounting_policy": (
        r"\becl\b",
        r"\bcecl\b",
        r"\bstaging\b",
        r"\bdpd\b",
        r"\bcure[_ -]?period\b",
        r"\bforecast[_ -]?horizon\b",
        r"\breversion[_ -]?method\b",
    ),
    "binding_default": (
        r"\bann[_ -]?factor\b",
        r"\bannuali[sz]ation\b",
        r"\bliquidity[_ -]?tier\b",
        r"\btier[_ -]?thresholds\b",
        r"\blaguerre\b",
        r"\buse[_ -]?parallel\b",
        r"\bbasis[_ -]?degree\b",
        r"\bdefault[_ -]?currency\b",
    ),
}

NUMBER_RE = re.compile(
    r"(?<![A-Za-z0-9_])[-+]?(?:\d+\.\d+|\d+)(?:e[-+]?\d+)?(?![A-Za-z0-9_])",
    re.IGNORECASE,
)
STRING_RE = re.compile(r'"[^"\n]{2,}"|\'[^\'\n]{2,}\'')
RUST_TEST_ATTRIBUTE_RE = re.compile(r"^\s*#\[\s*(?:[A-Za-z_]\w*::)*test\b[^\]]*\]\s*$")
RUST_CFG_TEST_RE = re.compile(r"^\s*#\[\s*cfg\s*\([^\]]*\btest\b[^\]]*\)\s*\]\s*$")


@dataclass(frozen=True)
class AllowlistEntry:
    """One line-match allowlist rule."""

    path: str
    category: str
    contains: str
    reason: str


@dataclass(frozen=True)
class Finding:
    """A single scanner match."""

    path: str
    line: int
    category: str
    status: str
    excerpt: str
    reason: str


@dataclass(frozen=True)
class Args:
    """Parsed CLI arguments."""

    roots: tuple[Path, ...]
    allowlist: Path
    output_format: str
    include_allowed: bool
    fail_on_candidates: bool


def parse_args() -> Args:
    """Parse command-line arguments."""
    parser = argparse.ArgumentParser(
        description="Find likely hard-coded external assumptions that should move to registries."
    )
    parser.add_argument(
        "roots",
        nargs="*",
        type=Path,
        help="workspace-relative files/directories to scan; defaults to production source roots",
    )
    parser.add_argument(
        "--allowlist",
        type=Path,
        default=DEFAULT_ALLOWLIST,
        help="workspace-relative allowlist JSON path",
    )
    parser.add_argument(
        "--format",
        choices=("text", "markdown", "json"),
        default="text",
        help="report format",
    )
    parser.add_argument(
        "--include-allowed",
        action="store_true",
        help="include allowlisted matches in the printed report",
    )
    parser.add_argument(
        "--fail-on-candidates",
        action="store_true",
        help="exit non-zero when candidate source-code matches are found",
    )
    ns = parser.parse_args()
    roots = tuple(ns.roots) if ns.roots else tuple(Path(root) for root in DEFAULT_ROOTS)
    return Args(
        roots=roots,
        allowlist=ns.allowlist,
        output_format=ns.format,
        include_allowed=ns.include_allowed,
        fail_on_candidates=ns.fail_on_candidates,
    )


def compile_patterns() -> dict[str, tuple[re.Pattern[str], ...]]:
    """Compile category regex patterns."""
    return {
        category: tuple(re.compile(pattern, re.IGNORECASE) for pattern in patterns)
        for category, patterns in CATEGORY_PATTERNS.items()
    }


def load_allowlist(path: Path) -> list[AllowlistEntry]:
    """Load allowlist rules from JSON."""
    full_path = ROOT / path
    if not full_path.exists():
        return []

    with full_path.open("r", encoding="utf-8") as handle:
        data: dict[str, Any] = json.load(handle)

    entries: list[AllowlistEntry] = []
    for raw in data.get("entries", []):
        entries.append(
            AllowlistEntry(
                path=str(raw["path"]),
                category=str(raw["category"]),
                contains=str(raw["contains"]),
                reason=str(raw["reason"]),
            )
        )
    return entries


def should_skip_dir(dirname: str) -> bool:
    """Return whether a directory should be skipped."""
    return dirname in EXCLUDED_DIRS or dirname.startswith(".")


def should_skip_path(path: Path) -> bool:
    """Return whether a file path should be skipped."""
    rel = path.relative_to(ROOT).as_posix()
    if rel.endswith("/tests.rs"):
        return True
    return any(part in rel for part in EXCLUDED_PATH_PARTS) or path.suffix not in SOURCE_EXTENSIONS


def iter_files(roots: tuple[Path, ...]) -> list[Path]:
    """Collect files under requested roots."""
    files: list[Path] = []
    for root in roots:
        full_root = (ROOT / root).resolve()
        if not full_root.exists():
            continue
        if full_root.is_file():
            if not should_skip_path(full_root):
                files.append(full_root)
            continue
        for current_root, dirnames, filenames in os.walk(full_root):
            dirnames[:] = sorted(name for name in dirnames if not should_skip_dir(name))
            base = Path(current_root)
            for filename in sorted(filenames):
                path = base / filename
                if not should_skip_path(path):
                    files.append(path)
    return sorted(files)


def categories_for_line(line: str, patterns: dict[str, tuple[re.Pattern[str], ...]]) -> list[str]:
    """Return matching categories for a source line."""
    return [
        category
        for category, compiled_patterns in patterns.items()
        if any(pattern.search(line) for pattern in compiled_patterns)
    ]


def has_assumption_value(line: str, categories: list[str]) -> bool:
    """Return whether a matched line carries a value worth reporting."""
    if "rating_agency" in categories:
        return True
    if categories == ["structured_credit"]:
        return bool(NUMBER_RE.search(line))
    return bool(NUMBER_RE.search(line) or STRING_RE.search(line))


def is_comment_only(line: str) -> bool:
    """Return whether a line is only source commentary."""
    stripped = line.lstrip()
    return stripped.startswith(("//", "///", "/*", "*", "*/", "# "))


def rust_brace_delta(line: str) -> int:
    """Return a rough Rust brace-depth delta for test-block skipping."""
    return line.count("{") - line.count("}")


def iter_scannable_lines(path: Path) -> list[tuple[int, str]]:
    """Return source lines, excluding comments and inline Rust tests."""
    lines: list[tuple[int, str]] = []
    if path.suffix != ".rs":
        with path.open("r", encoding="utf-8", errors="ignore") as handle:
            for line_number, raw_line in enumerate(handle, start=1):
                if not is_comment_only(raw_line):
                    lines.append((line_number, raw_line))
        return lines

    skipping_item = False
    item_started = False
    brace_depth = 0

    with path.open("r", encoding="utf-8", errors="ignore") as handle:
        for line_number, raw_line in enumerate(handle, start=1):
            if skipping_item:
                if not item_started:
                    if "{" in raw_line:
                        item_started = True
                        brace_depth = rust_brace_delta(raw_line)
                        if brace_depth <= 0:
                            skipping_item = False
                            item_started = False
                            brace_depth = 0
                    elif ";" in raw_line:
                        skipping_item = False
                else:
                    brace_depth += rust_brace_delta(raw_line)
                    if brace_depth <= 0:
                        skipping_item = False
                        item_started = False
                        brace_depth = 0
                continue

            if RUST_TEST_ATTRIBUTE_RE.match(raw_line) or RUST_CFG_TEST_RE.match(raw_line):
                skipping_item = True
                item_started = False
                brace_depth = 0
                continue

            if not is_comment_only(raw_line):
                lines.append((line_number, raw_line))
    return lines


def allowlist_reason(path: str, category: str, line: str, allowlist: list[AllowlistEntry]) -> str | None:
    """Return the allowlist reason when a match is allowed."""
    for entry in allowlist:
        if entry.path == path and entry.category == category and entry.contains in line:
            return entry.reason
    return None


def classify_status(path: str, category: str, line: str, allowlist: list[AllowlistEntry]) -> tuple[str, str]:
    """Classify a finding as candidate, registry, or allowlisted."""
    reason = allowlist_reason(path, category, line, allowlist)
    if reason is not None:
        return "allowlisted", reason
    if any(part in path for part in REGISTRY_CODE_PATH_PARTS):
        return "registry_code", "Registry loader or validation code for external assumptions."
    if "/data/" in path and path.endswith(".json"):
        return "registry_data", "Existing JSON registry/data file; verify it is canonical and not duplicated."
    return "candidate", "Move to a registry/configuration or add a documented allowlist entry."


def scan_file(
    path: Path, patterns: dict[str, tuple[re.Pattern[str], ...]], allowlist: list[AllowlistEntry]
) -> list[Finding]:
    """Scan one file and return findings."""
    rel = path.relative_to(ROOT).as_posix()
    findings: list[Finding] = []
    for line_number, raw_line in iter_scannable_lines(path):
        line = raw_line.strip()
        if not line:
            continue
        categories = categories_for_line(line, patterns)
        if not categories or not has_assumption_value(line, categories):
            continue
        for category in categories:
            status, reason = classify_status(rel, category, line, allowlist)
            findings.append(
                Finding(
                    path=rel,
                    line=line_number,
                    category=category,
                    status=status,
                    excerpt=line[:180],
                    reason=reason,
                )
            )
    return findings


def collect_findings(args: Args) -> list[Finding]:
    """Collect findings from requested roots."""
    patterns = compile_patterns()
    allowlist = load_allowlist(args.allowlist)
    findings: list[Finding] = []
    for path in iter_files(args.roots):
        findings.extend(scan_file(path, patterns, allowlist))
    return sorted(findings, key=lambda item: (item.status, item.category, item.path, item.line, item.excerpt))


def status_counts(findings: list[Finding]) -> dict[str, int]:
    """Count findings by status."""
    counts: dict[str, int] = {}
    for finding in findings:
        counts[finding.status] = counts.get(finding.status, 0) + 1
    return counts


def visible_findings(findings: list[Finding], include_allowed: bool) -> list[Finding]:
    """Filter findings for display."""
    if include_allowed:
        return findings
    return [finding for finding in findings if finding.status != "allowlisted"]


def render_text(findings: list[Finding], include_allowed: bool) -> str:
    """Render a plain-text report."""
    visible = visible_findings(findings, include_allowed)
    counts = status_counts(findings)
    lines = [
        "Hard-coded external-assumption audit",
        "",
        "Status counts:",
        *(f"  {status}: {count}" for status, count in sorted(counts.items())),
        "",
        f"Showing {len(visible)} of {len(findings)} finding(s).",
        "",
        f"{'STATUS':<14} {'CATEGORY':<22} LOCATION",
        f"{'------':<14} {'--------':<22} --------",
    ]
    for finding in visible:
        location = f"{finding.path}:{finding.line}"
        lines.append(f"{finding.status:<14} {finding.category:<22} {location}")
        lines.append(f"  {finding.excerpt}")
    return "\n".join(lines)


def render_markdown(findings: list[Finding], include_allowed: bool) -> str:
    """Render a markdown report."""
    visible = visible_findings(findings, include_allowed)
    counts = status_counts(findings)
    lines = [
        "# Hard-Coded External-Assumption Audit",
        "",
        "## Status Counts",
        "",
        "| Status | Count |",
        "|---|---:|",
        *(f"| `{status}` | {count} |" for status, count in sorted(counts.items())),
        "",
        "## Findings",
        "",
        "| Status | Category | Location | Excerpt |",
        "|---|---|---|---|",
    ]
    for finding in visible:
        excerpt = finding.excerpt.replace("|", "\\|")
        lines.append(f"| `{finding.status}` | `{finding.category}` | `{finding.path}:{finding.line}` | `{excerpt}` |")
    return "\n".join(lines)


def render_json(findings: list[Finding]) -> str:
    """Render a JSON report."""
    payload = {
        "schema_version": "finstack_quant.hardcoded_assumptions_audit/1",
        "counts": status_counts(findings),
        "findings": [
            {
                "path": finding.path,
                "line": finding.line,
                "category": finding.category,
                "status": finding.status,
                "excerpt": finding.excerpt,
                "reason": finding.reason,
            }
            for finding in findings
        ],
    }
    return json.dumps(payload, indent=2, sort_keys=True)


def main() -> int:
    """Run the audit."""
    args = parse_args()
    findings = collect_findings(args)
    if args.output_format == "json":
        print(render_json(findings))
    elif args.output_format == "markdown":
        print(render_markdown(findings, args.include_allowed))
    else:
        print(render_text(findings, args.include_allowed))

    candidate_count = status_counts(findings).get("candidate", 0)
    return 1 if args.fail_on_candidates and candidate_count else 0


if __name__ == "__main__":
    sys.exit(main())
