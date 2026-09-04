"""Mapped labs expose instrument families only after their fixture stage exists."""

from pathlib import Path

from build_labs import published_notebooks, source_notebooks
from check_curriculum import validate_instrument_fixture_usage
from common import curriculum_manifest, lesson_notebook_names
import nbformat


def _write_notebook(path: Path, source: str, dependencies: list[str] | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    notebook = nbformat.v4.new_notebook(
        cells=[nbformat.v4.new_code_cell(source)],
        metadata={"analyst_dependencies": dependencies or []},
    )
    nbformat.write(notebook, path)


def _records(*, labs: list[str] | None = None, examples: list[str] | None = None) -> list[dict]:
    return [
        {
            "id": "1.1",
            "requires": [],
            "fixtures": ["foundations"],
            "labs": [],
            "examples": [],
        },
        {
            "id": "2.5",
            "requires": ["1.1"],
            "fixtures": ["base"],
            "labs": [],
            "examples": [],
        },
        {
            "id": "3.3",
            "requires": ["2.5"],
            "fixtures": ["base"],
            "labs": labs or [],
            "examples": examples or [],
        },
    ]


def test_mapped_example_cannot_import_a_later_instrument_family(tmp_path: Path) -> None:
    _write_notebook(
        tmp_path / "early.ipynb",
        "from _shared import instrument_fixtures as fixtures\nfixtures.term_loan(0)\n",
    )

    errors = validate_instrument_fixture_usage(_records(examples=["early.ipynb"]), tmp_path)

    assert errors == ["3.3: early.ipynb imports instrument fixture term_loan before fixtures ['common'] are available"]


def test_mapped_lab_checks_recursive_notebook_dependencies(tmp_path: Path) -> None:
    _write_notebook(tmp_path / "main.ipynb", "result = 1\n", ["later.ipynb"])
    _write_notebook(
        tmp_path / "later.ipynb",
        "from _shared.instrument_fixtures import variance_swap\nvariance_swap(0)\n",
    )

    errors = validate_instrument_fixture_usage(_records(labs=["main.ipynb"]), tmp_path)

    assert errors == [
        "3.3: later.ipynb imports instrument fixture variance_swap before fixtures ['volatility'] are available"
    ]


def test_available_instrument_family_and_generic_envelope_pass(tmp_path: Path) -> None:
    _write_notebook(
        tmp_path / "allowed.ipynb",
        "from _shared.instrument_fixtures import fixed_bond, instrument_envelope\n"
        "_, bond = fixed_bond(0)\n"
        "instrument_envelope(bond)\n",
    )

    assert validate_instrument_fixture_usage(_records(labs=["allowed.ipynb"]), tmp_path) == []


def test_shared_package_wildcard_uses_only_actual_reexports(tmp_path: Path) -> None:
    _write_notebook(tmp_path / "allowed.ipynb", "from _shared import *\nacme_bond()\n")

    assert validate_instrument_fixture_usage(_records(labs=["allowed.ipynb"]), tmp_path) == []


def test_private_fixture_helpers_and_shared_dates_are_rejected(tmp_path: Path) -> None:
    _write_notebook(
        tmp_path / "invalid.ipynb",
        "from _shared import instrument_fixtures as fixtures\nfixtures._pool_assets('early')\nvalue = fixtures.AS_OF\n",
    )

    errors = validate_instrument_fixture_usage(_records(labs=["invalid.ipynb"]), tmp_path)

    assert errors == [
        "3.3: invalid.ipynb must inline AS_OF instead of importing it",
        "3.3: invalid.ipynb imports unclassified instrument fixture _pool_assets",
    ]


def test_capstone_variant_labs_cannot_import_the_other_track(tmp_path: Path) -> None:
    _write_notebook(
        tmp_path / "credit.ipynb",
        "from _shared.instrument_fixtures import variance_swap\nvariance_swap(0)\n",
    )
    _write_notebook(
        tmp_path / "volatility.ipynb",
        "from _shared.instrument_fixtures import revolver\nrevolver(0)\n",
    )
    records = [
        {"id": "4.5", "requires": [], "fixtures": ["common"], "labs": [], "examples": []},
        {"id": "C1", "requires": ["4.5"], "fixtures": ["credit"], "labs": [], "examples": []},
        {"id": "V1", "requires": ["4.5"], "fixtures": ["volatility"], "labs": [], "examples": []},
        {
            "id": "capstone",
            "requires": ["4.5"],
            "fixtures": ["common"],
            "labs": [],
            "examples": [],
            "variants": {"credit": ["C1"], "volatility": ["V1"]},
            "variant_labs": {"credit": ["credit.ipynb"], "volatility": ["volatility.ipynb"]},
        },
    ]

    errors = validate_instrument_fixture_usage(records, tmp_path)

    assert errors == [
        "capstone: credit.ipynb imports instrument fixture variance_swap before fixtures ['volatility'] are available",
        "capstone: volatility.ipynb imports instrument fixture revolver before fixtures ['credit'] are available",
    ]


def test_scale_notebook_is_executed_but_not_published() -> None:
    scale = "05_portfolio/multi_asset_portfolio_at_scale.ipynb"
    manifest = curriculum_manifest(Path(__file__).parents[1])
    mapped = {notebook for lesson in manifest["lesson"] for notebook in lesson_notebook_names(lesson)}

    assert scale in source_notebooks()
    assert scale not in mapped
    assert scale not in published_notebooks()
    capstone = next(lesson for lesson in manifest["lesson"] if lesson["id"] == "capstone")
    assert {lab for labs in capstone["variant_labs"].values() for lab in labs} <= published_notebooks()
    assert validate_instrument_fixture_usage(manifest["lesson"]) == []
