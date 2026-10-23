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
        reference_data.date_rows(path)


SIFMA_HEADER = "year,month,class_a,class_b,class_c,class_d\n"


def test_sifma_rows_sort_and_render_unpublished_classes(tmp_path: Path) -> None:
    """Sort settlement rows by month and render unpublished classes as None."""
    path = tmp_path / "sifma.csv"
    path.write_text(SIFMA_HEADER + "2025,2,13,,,\n2025,1,14,,,\n", encoding="utf-8")
    rows = reference_data.sifma_rows(path)
    assert [(year, month) for year, month, _ in rows] == [(2025, 1), (2025, 2)]
    assert rows[1][2] == (13, None, None, None)
    assert "    (2025, 2, [Some(13), None, None, None])," in reference_data.render_sifma(rows)


@pytest.mark.parametrize(
    ("text", "message"),
    [
        ("year,month,class_a\n", "expected header"),
        (SIFMA_HEADER + "2025,2,30,,,\n", "has only 28 days"),
        (SIFMA_HEADER + "2025,1,,,,\n", "at least one class"),
        (SIFMA_HEADER + "2025,1,9,,,\n2025,1,10,,,\n", "duplicate year/month row"),
        (SIFMA_HEADER + "2025,13,9,,,\n", "month must be 1-12"),
        (SIFMA_HEADER + "2025,1,32,,,\n", "must be 1-31"),
    ],
)
def test_sifma_rejects_invalid_rows(tmp_path: Path, text: str, message: str) -> None:
    """Reject malformed or impossible settlement rows before any Rust is written."""
    path = tmp_path / "sifma.csv"
    path.write_text(text, encoding="utf-8")
    with pytest.raises(ValueError, match=message):
        reference_data.sifma_rows(path)


@pytest.fixture
def generation_roots(tmp_path: Path) -> tuple[Path, Path]:
    """Copy authoritative inputs into isolated Rust and WASM output roots."""
    crate_root = tmp_path / "core"
    wasm_root = tmp_path / "wasm"
    (crate_root / "data").mkdir(parents=True)
    (wasm_root / "src/api/core").mkdir(parents=True)
    for filename in [
        "iso_4217.csv",
        "chinese_new_year.csv",
        "sifma_settlements.csv",
        *[f"{name}.csv" for name in reference_data.FESTIVAL_TABLES],
    ]:
        (crate_root / "data" / filename).write_bytes((reference_data.CRATE_ROOT / "data" / filename).read_bytes())
    return crate_root, wasm_root


def run_generator(roots: tuple[Path, Path], *, check: bool = False) -> None:
    """Invoke the production CLI with both destinations explicitly isolated."""
    crate_root, wasm_root = roots
    arguments = ["generate", "--crate-root", str(crate_root), "--wasm-root", str(wasm_root)]
    if check:
        arguments.append("--check")
    with patch("sys.argv", arguments):
        reference_data.main()


@pytest.mark.parametrize(
    "table",
    [
        "currency_generated.rs",
        "cny_generated.rs",
        "festivals_generated.rs",
        "sifma_settlements_generated.rs",
        "currency_statics.rs",
    ],
)
def test_check_detects_drift_without_overwriting_it(generation_roots: tuple[Path, Path], table: str) -> None:
    """Report stale Rust or WASM files without changing their bytes."""
    crate_root, wasm_root = generation_roots
    run_generator(generation_roots)
    directory = wasm_root / "src/api/core" if table == "currency_statics.rs" else crate_root / "src/generated"
    output = directory / table
    output.write_text("stale table\n")
    with pytest.raises(SystemExit, match="Stale generated reference data") as error:
        run_generator(generation_roots, check=True)
    assert str(output) in str(error.value)
    assert output.read_text() == "stale table\n"


def test_generation_is_isolated_and_idempotent(generation_roots: tuple[Path, Path], tmp_path: Path) -> None:
    """Explicit roots override defaults, and unchanged output is never rewritten."""
    with patch.object(reference_data, "CRATE_ROOT", tmp_path / "unselected"):
        run_generator(generation_roots)
        outputs = [path for root in generation_roots for path in root.rglob("*.rs")]
        before = {path: (path.read_bytes(), path.stat().st_mtime_ns) for path in outputs}
        run_generator(generation_roots)
        run_generator(generation_roots, check=True)
    assert len(outputs) == 5
    assert before == {path: (path.read_bytes(), path.stat().st_mtime_ns) for path in outputs}
    assert not (tmp_path / "unselected").exists()


@pytest.mark.parametrize("name", reference_data.FESTIVAL_TABLES)
def test_every_festival_input_requires_the_complete_year_range(generation_roots: tuple[Path, Path], name: str) -> None:
    """Reject a missing year before writing any Rust or WASM output."""
    crate_root, wasm_root = generation_roots
    path = crate_root / "data" / f"{name}.csv"
    path.write_text("\n".join(path.read_text().splitlines()[:-1]) + "\n")
    with pytest.raises(ValueError, match="one date per year"):
        run_generator(generation_roots)
    assert not list(crate_root.rglob("*.rs"))
    assert not list(wasm_root.rglob("*.rs"))
