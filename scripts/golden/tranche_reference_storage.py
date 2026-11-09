"""Store readable tranche regression pins separately from baseline reports."""

from __future__ import annotations

import json
from pathlib import Path
import tempfile
from typing import Any

_REPORT_KEYS = frozenset({"baseline_errors_with_uncertainty", "baseline_grid_deviations", "baseline_acceptance"})


def _report_path(input_path: Path, output_path: Path) -> Path:
    """Derive the companion report and reject destinations aliasing the source export."""
    report_path = output_path.with_name(f"{output_path.stem}.report.json")
    if input_path.resolve() in (output_path.resolve(), report_path.resolve()):
        raise ValueError("regression pins and companion report must not overwrite the input export")
    return report_path


def regression_pins(value: Any) -> Any:
    """Copy JSON data without the three report-only fields at any depth.

    Args:
        value: Complete baseline report; dictionaries and lists are copied.

    Returns:
        The report with only the three diagnostic keys removed recursively.
        All numerical pins, gates and provenance retain their original values.
    """
    if isinstance(value, dict):
        return {key: regression_pins(item) for key, item in value.items() if key not in _REPORT_KEYS}
    if isinstance(value, list):
        return [regression_pins(item) for item in value]
    return value


def _format_json(value: Any, depth: int = 0, *, conditional: bool = False) -> str:
    """Keep scalar arrays and each conditional node on one reviewable line."""
    indent = "  " * depth
    child_indent = indent + "  "
    if isinstance(value, dict) and value:
        fields = (
            f"{child_indent}{json.dumps(key)}: {_format_json(item, depth + 1, conditional=key == 'conditional')}"
            for key, item in value.items()
        )
        return "{\n" + ",\n".join(fields) + "\n" + indent + "}"
    if isinstance(value, list) and value and any(isinstance(item, (dict, list)) for item in value):
        items = (
            json.dumps(item, separators=(",", ":"), allow_nan=False) if conditional else _format_json(item, depth + 1)
            for item in value
        )
        return "[\n" + ",\n".join(child_indent + item for item in items) + "\n" + indent + "]"
    return json.dumps(value, separators=(", ", ": "), allow_nan=False)


def write_json(path: Path, value: Any) -> None:
    """Atomically replace a UTF-8 JSON artifact, preserving all supplied data.

    Args:
        path: Destination file; missing parent directories are created.
        value: JSON-compatible data. Conditional nodes occupy one line each;
            scalar arrays remain inline.

    Raises:
        ValueError: A number is NaN or infinite; no artifact is changed.
        TypeError: A supplied value cannot be encoded as JSON.
        OSError: The destination cannot be written or replaced.
    """
    encoded = _format_json(value) + "\n"
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", dir=path.parent, prefix=path.name + ".", suffix=".tmp", delete=False
        ) as handle:
            temporary = Path(handle.name)
            handle.write(encoded)
        temporary.replace(path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
