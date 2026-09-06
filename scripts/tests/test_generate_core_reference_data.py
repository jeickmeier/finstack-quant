"""Regression checks for maintained reference-data validation and drift checks."""

import importlib.util
from pathlib import Path
from unittest.mock import patch

import pytest

spec = importlib.util.spec_from_file_location(
    "reference_data", Path(__file__).resolve().parents[2] / "finstack-quant/core/build/generate_reference_data.py"
)
reference_data = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference_data)


def test_duplicate_numeric_currency_rejected(tmp_path: Path) -> None:
    """Reject duplicate numeric codes even when the alphabetic codes differ."""
    path = tmp_path / "currencies.csv"
    path.write_text("code,numeric,minor_units,name\nUSD,840,2,Dollar\nEUR,840,2,Euro\n")
    with pytest.raises(ValueError, match="duplicate currency"):
        reference_data.currency_rows(path)


def test_missing_calendar_year_rejected(tmp_path: Path) -> None:
    """Reject incomplete tables before they can replace calendar reference data."""
    path = tmp_path / "dates.csv"
    path.write_text("year,month,day\n2025,1,29\n")
    with pytest.raises(ValueError, match="one date per year"):
        reference_data.cny_rows(path)


def test_check_detects_drift_without_overwriting_it(tmp_path: Path) -> None:
    """Report stale generated files without modifying them during verification."""
    (tmp_path / "src/generated").mkdir(parents=True)
    (tmp_path / "data").mkdir()
    for filename in ["iso_4217.csv", "chinese_new_year.csv"]:
        (tmp_path / "data" / filename).write_bytes((reference_data.CRATE_ROOT / "data" / filename).read_bytes())
    with patch.object(reference_data, "CRATE_ROOT", tmp_path), patch("sys.argv", ["generate"]):
        reference_data.main()
        output = tmp_path / "src/generated/currency_generated.rs"
        output.write_text("stale table\n")
        with patch("sys.argv", ["generate", "--check"]), pytest.raises(SystemExit, match="Stale generated"):
            reference_data.main()
        assert output.read_text() == "stale table\n"
