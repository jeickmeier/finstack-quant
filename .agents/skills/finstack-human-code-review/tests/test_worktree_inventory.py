"""Stdlib-only regression checks for the read-only Git inventory helper."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "worktree_inventory.py"


class WorktreeInventoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name) / "repository"
        self.repo.mkdir()
        self.git("init", "--quiet")
        self.git("config", "user.name", "Example Developer")
        self.git("config", "user.email", "example@example.invalid")

    def git(self, *args: str) -> None:
        subprocess.run(["git", "-C", str(self.repo), *args], check=True, capture_output=True)

    def inventory(self) -> dict:
        result = subprocess.run(
            [sys.executable, str(SCRIPT), "--repo", str(self.repo), "--format", "json"],
            check=True,
            capture_output=True,
            text=True,
        )
        return json.loads(result.stdout)

    def commit_initial(self) -> None:
        (self.repo / "tracked.py").write_text("value = 1\n", encoding="utf-8")
        (self.repo / "old module.py").write_text("def run(): pass\n", encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "--quiet", "-m", "Initial")

    def test_unborn_repo_and_staged_file(self) -> None:
        (self.repo / "new.py").write_text("print('new')\n", encoding="utf-8")
        self.git("add", "new.py")
        result = self.inventory()
        self.assertFalse(result["has_head_commit"])
        self.assertEqual(result["counts"]["staged"], 1)
        self.assertEqual(result["entries"][0]["path"], "new.py")

    def test_staged_unstaged_untracked_and_rename(self) -> None:
        self.commit_initial()
        (self.repo / "tracked.py").write_text("value = 2\n", encoding="utf-8")
        self.git("add", "tracked.py")
        (self.repo / "tracked.py").write_text("value = 3\n", encoding="utf-8")
        self.git("mv", "old module.py", "renamed module.py")
        (self.repo / "untracked file.py").write_text("# new\n", encoding="utf-8")
        result = self.inventory()
        by_path = {item["path"]: item for item in result["entries"]}
        self.assertTrue(result["has_head_commit"])
        self.assertTrue(by_path["tracked.py"]["staged"])
        self.assertTrue(by_path["tracked.py"]["unstaged"])
        self.assertEqual(by_path["renamed module.py"]["old_path"], "old module.py")
        self.assertTrue(by_path["untracked file.py"]["untracked"])
        self.assertEqual(result["counts"]["changed_paths"], 3)

    def test_clean_repository_and_explicit_deletion(self) -> None:
        self.commit_initial()
        self.assertEqual(self.inventory()["counts"]["changed_paths"], 0)
        self.git("rm", "old module.py")
        by_path = {item["path"]: item for item in self.inventory()["entries"]}
        self.assertTrue(by_path["old module.py"]["deleted"])
        self.assertTrue(by_path["old module.py"]["staged"])

    def test_unicode_name(self) -> None:
        (self.repo / "résumé one.py").write_text("x = 1\n", encoding="utf-8")
        self.assertEqual(self.inventory()["entries"][0]["path"], "résumé one.py")


if __name__ == "__main__":
    unittest.main()
