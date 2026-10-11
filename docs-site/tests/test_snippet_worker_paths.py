"""Snippet worker paths stay inside the asset root."""

from __future__ import annotations

import json
from pathlib import Path
import sys

import pytest
import snippet_worker


def test_parent_segment_ids_are_rejected_before_execution(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """A lesson or block id of ``..`` never runs the block or writes a result."""
    request = tmp_path / "request.json"
    result = tmp_path / "result.json"
    request.write_text(
        json.dumps({
            "lesson_id": "..",
            "blocks": [
                {
                    "id": "proof",
                    "role": "build",
                    "code": "raise SystemExit('executed')\n",
                    "line": 1,
                }
            ],
            "capture": ["proof"],
        }),
        encoding="utf-8",
    )
    monkeypatch.setattr(sys, "argv", ["snippet_worker.py", str(request), str(result)])

    with pytest.raises(RuntimeError, match="lesson_id"):
        snippet_worker.main()

    request.write_text(
        json.dumps({
            "lesson_id": "2.6",
            "blocks": [
                {
                    "id": "..",
                    "role": "build",
                    "code": "raise SystemExit('executed')\n",
                    "line": 1,
                }
            ],
            "capture": [".."],
        }),
        encoding="utf-8",
    )
    with pytest.raises(RuntimeError, match="block id"):
        snippet_worker.main()

    assert not result.exists()


def test_asset_directory_cannot_escape_the_root(tmp_path: Path) -> None:
    """Resolved asset directories that leave the root are refused, and outside files stay."""
    outside = tmp_path / "outside"
    block = outside / "block"
    block.mkdir(parents=True)
    marker = block / "keep.txt"
    marker.write_text("keep", encoding="utf-8")
    asset_root = tmp_path / "assets"
    asset_root.mkdir()
    (asset_root / "2.6").symlink_to(outside, target_is_directory=True)

    with pytest.raises(RuntimeError, match="escapes"):
        snippet_worker.confined_directory(asset_root, "2.6", "block")

    assert marker.read_text(encoding="utf-8") == "keep"
    confined = snippet_worker.confined_directory(asset_root, "2.8", "proof")
    assert confined.is_relative_to(asset_root.resolve())


def test_captured_file_is_not_written_through_an_escaping_symlink(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """A copy target that resolves outside the asset root is refused."""
    outside = tmp_path / "outside"
    outside.mkdir()
    marker = outside / "keep.txt"
    marker.write_text("keep", encoding="utf-8")
    asset_root = tmp_path / "assets"
    asset_path = asset_root / "2.6" / "proof"
    asset_path.mkdir(parents=True)
    (asset_path / "escape").symlink_to(outside, target_is_directory=True)
    work = tmp_path / "work"
    work.mkdir()
    monkeypatch.chdir(work)
    source = work / "escape" / "keep.txt"
    monkeypatch.setattr(snippet_worker, "files", lambda: {source: "replaced"})

    with pytest.raises(RuntimeError, match="escapes"):
        snippet_worker.capture_files(
            {},
            asset_path,
            "/snippet-assets/2.6/proof/",
            asset_root,
            publish_assets=True,
        )

    assert marker.read_text(encoding="utf-8") == "keep"
