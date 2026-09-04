"""Snippet evidence must remain bound to its published asset files."""

import hashlib
import json
from pathlib import Path

import check_curriculum
from check_curriculum import check_evidence
from common import digest
import pytest
from run_lesson_snippets import Block, canonical_validation_manifest, run_process

ASSET = "/snippet-assets/2.6/proof/chart.svg"


def _write_asset(site: Path, asset: str, data: bytes) -> None:
    path = site / "public" / asset.removeprefix("/")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def _errors(
    site: Path,
    monkeypatch: pytest.MonkeyPatch,
    assets: list[str],
    hashes: dict[str, str],
    captured_block_id: str = "proof",
) -> list[str]:
    source = site / "lesson.mdx"
    source.write_text("lesson\n")
    block = Block("proof", "build", "value = 42\n", 1)
    runtime = {"python": "test"}
    monkeypatch.setattr(check_curriculum, "lesson_notebooks", lambda _record: {})
    monkeypatch.setattr(check_curriculum, "runtime_identity", lambda: runtime)
    monkeypatch.setattr(check_curriculum, "snippet_provenance", dict)
    monkeypatch.setattr(check_curriculum, "canonical_execution_blocks", lambda *_args: [block])
    evidence = {
        "status": "passed",
        "source_sha256": digest(source),
        "fixtures_sha256": "fixtures",
        "notebooks_sha256": {},
        "runtime": runtime,
        "canonical_validation": {
            "status": "passed",
            **canonical_validation_manifest([block]),
        },
        "blocks": [
            {
                "id": captured_block_id,
                "role": "build",
                "stdout": "",
                "assets": assets,
                "asset_sha256": hashes,
            }
        ],
    }
    output = site / ".build" / "snippets" / "2.6.json"
    output.parent.mkdir(parents=True)
    output.write_text(json.dumps(evidence))
    record = {"id": "2.6", "path": source.name, "labs": []}
    return check_evidence(record, source, [block], site, {}, "fixtures")


def test_current_snippet_asset_evidence_is_accepted(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    data = b"current"
    _write_asset(tmp_path, ASSET, data)

    assert _errors(tmp_path, monkeypatch, [ASSET], {ASSET: hashlib.sha256(data).hexdigest()}) == []


def test_deleted_snippet_asset_invalidates_evidence(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    errors = _errors(tmp_path, monkeypatch, [ASSET], {ASSET: hashlib.sha256(b"deleted").hexdigest()})

    assert any("missing snippet asset" in error for error in errors)


def test_corrupt_snippet_asset_invalidates_evidence(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    _write_asset(tmp_path, ASSET, b"corrupt")

    errors = _errors(tmp_path, monkeypatch, [ASSET], {ASSET: hashlib.sha256(b"original").hexdigest()})

    assert any("snippet asset hash mismatch" in error for error in errors)


def test_snippet_asset_list_and_hash_keys_must_match(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    other = "/snippet-assets/2.6/proof/other.svg"
    data = b"current"
    _write_asset(tmp_path, ASSET, data)
    _write_asset(tmp_path, other, data)

    errors = _errors(tmp_path, monkeypatch, [ASSET], {other: hashlib.sha256(data).hexdigest()})

    assert any("asset paths and asset_sha256 keys differ" in error for error in errors)


def test_snippet_asset_path_cannot_traverse_its_block_directory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    traversal = "/snippet-assets/2.6/proof/../../outside.svg"
    data = b"outside"
    _write_asset(tmp_path, traversal, data)

    errors = _errors(tmp_path, monkeypatch, [traversal], {traversal: hashlib.sha256(data).hexdigest()})

    assert any("asset path must remain inside its block directory" in error for error in errors)


def test_snippet_asset_block_id_cannot_traverse_the_lesson_directory(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    traversal = "/snippet-assets/2.6/../outside/chart.svg"
    data = b"outside"
    _write_asset(tmp_path, traversal, data)

    errors = _errors(
        tmp_path,
        monkeypatch,
        [traversal],
        {traversal: hashlib.sha256(data).hexdigest()},
        captured_block_id="../outside",
    )

    assert any("invalid snippet block ID" in error for error in errors)


def test_snippet_worker_prunes_obsolete_files_from_block_directory(isolated_snippet_site: Path) -> None:
    directory = isolated_snippet_site / "public" / "snippet-assets" / "2.6" / "proof"
    obsolete = directory / "old" / "obsolete.svg"
    obsolete.parent.mkdir(parents=True)
    obsolete.write_text("obsolete")
    block = Block(
        "proof",
        "build",
        "from pathlib import Path\nPath('current.svg').write_text('<svg>current</svg>')\n",
        1,
    )

    result = run_process("2.6", [block], ["proof"], 30)

    assert result[0]["assets"] == ["/snippet-assets/2.6/proof/current.svg"]
    assert (directory / "current.svg").read_text() == "<svg>current</svg>"
    assert not obsolete.exists()


def test_snippet_worker_orders_captured_files_by_path() -> None:
    block = Block(
        "proof",
        "build",
        ("from pathlib import Path\nPath('z-last.csv').write_text('z')\nPath('a-first.csv').write_text('a')\n"),
        1,
    )

    result = run_process("2.6", [block], ["proof"], 30)

    assert result[0]["assets"] == [
        "/snippet-assets/2.6/proof/a-first.csv",
        "/snippet-assets/2.6/proof/z-last.csv",
    ]


def test_snippet_worker_orders_figures_and_files_together() -> None:
    block = Block(
        "proof",
        "build",
        (
            "from pathlib import Path\n"
            "import matplotlib.pyplot as plt\n"
            "Path('a-first.svg').write_text('<svg>first</svg>')\n"
            "plt.figure(1)\n"
            "plt.plot([0, 1], [0, 1])\n"
        ),
        1,
    )

    result = run_process("2.6", [block], ["proof"], 30)

    assert result[0]["assets"] == [
        "/snippet-assets/2.6/proof/a-first.svg",
        "/snippet-assets/2.6/proof/figure-1.png",
    ]
    assert result[0]["assets"] == list(result[0]["asset_sha256"])


def test_unrecorded_asset_and_removed_block_directory_invalidate_evidence(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    data = b"current"
    stale = "/snippet-assets/2.6/removed/stale.svg"
    _write_asset(tmp_path, ASSET, data)
    _write_asset(tmp_path, stale, b"stale")

    errors = _errors(tmp_path, monkeypatch, [ASSET], {ASSET: hashlib.sha256(data).hexdigest()})

    assert any(stale in error and "unexpected" in error for error in errors)
    assert any("obsolete snippet block asset directories: ['removed']" in error for error in errors)


def test_successful_publication_replaces_the_whole_lesson_asset_tree(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import run_lesson_snippets as runner

    monkeypatch.setattr(runner, "SITE", tmp_path)
    old = tmp_path / "public" / "snippet-assets" / "2.6" / "removed" / "stale.svg"
    old.parent.mkdir(parents=True)
    old.write_text("stale")
    staged_root = tmp_path / "staged"
    current = staged_root / "2.6" / "proof" / "current.svg"
    current.parent.mkdir(parents=True)
    current.write_text("current")

    runner.publish_lesson_assets("2.6", staged_root)

    published = tmp_path / "public" / "snippet-assets" / "2.6"
    assert (published / "proof" / "current.svg").read_text() == "current"
    assert not (published / "removed").exists()


def test_failed_staged_run_preserves_the_published_lesson_assets(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    import run_lesson_snippets as runner

    monkeypatch.setattr(runner, "SITE", tmp_path)
    published = tmp_path / "public" / "snippet-assets" / "2.6" / "proof" / "last-good.svg"
    published.parent.mkdir(parents=True)
    published.write_text("last good")

    def fail_staged_run() -> None:
        with runner.staged_snippet_assets("2.6") as staged_root:
            staged = staged_root / "2.6" / "proof" / "replacement.svg"
            staged.parent.mkdir(parents=True)
            staged.write_text("replacement")
            raise RuntimeError("execution failed")

    with pytest.raises(RuntimeError, match="execution failed"):
        fail_staged_run()

    assert published.read_text() == "last good"
