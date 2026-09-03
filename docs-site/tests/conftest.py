"""Load the standalone documentation build tools for focused tests."""

from pathlib import Path
import shutil
import sys

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "gen"))


@pytest.fixture(autouse=True)
def isolated_snippet_site(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Run the unchanged capture worker from a temporary site, never public assets."""
    import run_lesson_snippets as runner

    site = tmp_path / "snippet-site"
    worker = site / "gen" / "snippet_worker.py"
    worker.parent.mkdir(parents=True)
    shutil.copy2(runner.SITE / "gen" / "snippet_worker.py", worker)
    monkeypatch.setattr(runner, "SITE", site)
    monkeypatch.setattr(runner, "BUILD", site / ".build")
    return site
