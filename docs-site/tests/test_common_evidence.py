"""Runtime evidence must change with every executable Python dependency."""

import json
from pathlib import Path
from types import SimpleNamespace

import common
import pytest


def test_variant_labs_are_included_in_lesson_source_fingerprints(tmp_path: Path) -> None:
    for name in ("credit.ipynb", "volatility.ipynb"):
        (tmp_path / name).write_text(json.dumps({"cells": [], "metadata": {"analyst_dependencies": []}}))
    record = {
        "labs": [],
        "examples": [],
        "variant_labs": {"credit": ["credit.ipynb"], "volatility": ["volatility.ipynb"]},
    }

    assert set(common.lesson_notebooks(record, tmp_path)) == {"credit.ipynb", "volatility.ipynb"}


def test_runtime_identity_changes_when_uv_lock_changes(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    package = tmp_path / "finstack-quant-py" / "finstack_quant"
    package.mkdir(parents=True)
    (package / "__init__.py").write_text("# package source\n")
    extension = tmp_path / "finstack_quant.so"
    extension.write_bytes(b"extension")
    lock = tmp_path / "uv.lock"
    lock.write_text("version = 1\n")
    monkeypatch.setattr(common, "REPO", tmp_path)
    monkeypatch.setattr(
        common.importlib,
        "import_module",
        lambda _: SimpleNamespace(__file__=str(extension), __version__="test"),
    )

    before = common.runtime_identity()
    lock.write_text("version = 2\n")
    after = common.runtime_identity()

    assert before != after


def test_fixture_digest_changes_when_external_pricer_defaults_change(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    notebooks = tmp_path / "finstack-quant-py" / "examples" / "notebooks"
    notebooks.mkdir(parents=True)
    (notebooks / "fixture.json").write_text('{"fixture": 1}\n')
    defaults = tmp_path / "finstack-quant" / "models" / "data" / "defaults" / "pricer_defaults.v1.json"
    defaults.parent.mkdir(parents=True)
    defaults.write_text('{"version": 1}\n')
    calibration = tmp_path / "finstack-quant" / "calibration" / "examples"
    calibration.mkdir(parents=True)
    (calibration / "curve.json").write_text('{"rate": 0.04}\n')
    monkeypatch.setattr(common, "REPO", tmp_path)
    monkeypatch.setattr(common, "NOTEBOOKS", notebooks)

    before = common.fixture_digest()
    defaults.write_text('{"version": 2}\n')
    after = common.fixture_digest()

    assert before != after


def test_publication_environment_pins_optional_benchmark_inputs(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    matplotlib_cache = tmp_path / "cache" / "matplotlib"
    matplotlib_cache.mkdir(parents=True)
    (matplotlib_cache / "fontlist-v-test.json").write_text("{}\n")
    monkeypatch.setenv("FINSTACK_RUN_XL_NOTEBOOK_BENCH", "1")
    monkeypatch.setenv("FINSTACK_RUN_EXTREME_ATTRIBUTION_NOTEBOOK_BENCH", "1")

    env = common.execution_environment(tmp_path)

    assert env["FINSTACK_RUN_XL_NOTEBOOK_BENCH"] == "0"
    assert env["FINSTACK_RUN_EXTREME_ATTRIBUTION_NOTEBOOK_BENCH"] == "0"
