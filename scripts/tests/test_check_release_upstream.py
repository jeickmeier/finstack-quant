"""Tests for the release-branch upstream check, against a scratch repository."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import shutil
import subprocess
import sys

import pytest

_SCRIPT_PATH = Path(__file__).parents[1] / "check_release_upstream.py"
_SPEC = importlib.util.spec_from_file_location("check_release_upstream", _SCRIPT_PATH)
assert _SPEC is not None
assert _SPEC.loader is not None
_MODULE = importlib.util.module_from_spec(_SPEC)
sys.modules[_SPEC.name] = _MODULE
_SPEC.loader.exec_module(_MODULE)
unmatched = _MODULE.unmatched
missing = _MODULE.missing


_GIT = shutil.which("git") or "git"


def _git(repo: Path, *args: str) -> str:
    return subprocess.run(  # noqa: S603 -- resolved git with fixed test arguments; no shell.
        [_GIT, *args], cwd=repo, check=True, capture_output=True, text=True
    ).stdout.strip()


def _commit(repo: Path, name: str, content: str, message: str) -> str:
    (repo / name).write_text(content, encoding="utf-8")
    _git(repo, "add", name)
    _git(repo, "commit", "-q", "-m", message)
    return _git(repo, "rev-parse", "HEAD")


@pytest.fixture
def repo(tmp_path: Path) -> Path:
    """master: base, then PRs #1-#3; release cut at base."""
    _git(tmp_path, "init", "-q", "-b", "master")
    _git(tmp_path, "config", "user.email", "t@example.com")
    _git(tmp_path, "config", "user.name", "t")
    _git(tmp_path, "config", "commit.gpgsign", "false")
    _commit(tmp_path, "base.txt", "base\n", "base")
    _git(tmp_path, "branch", "release")
    _commit(tmp_path, "a.txt", "a\n", "feat: a (#1)")
    _commit(tmp_path, "b.txt", "b\n", "fix: b (#2)")
    _commit(tmp_path, "c.txt", "c\n", "fix: c (#3)")
    _git(tmp_path, "switch", "-q", "release")
    return tmp_path


def _sha(repo: Path, subject: str) -> str:
    return _git(repo, "log", "master", "--format=%H", f"--grep={subject}", "--fixed-strings")


def test_clean_cherry_pick_passes(repo: Path) -> None:
    """A clean pick has the same patch as its master commit."""
    _git(repo, "cherry-pick", "-x", _sha(repo, "(#2)"))

    assert unmatched(repo, "master~3", "release", upstream="master") == []


def test_conflict_resolved_pick_passes_by_trailer(repo: Path) -> None:
    """A pick whose patch differs still passes through its -x trailer."""
    original = _sha(repo, "(#2)")
    _commit(repo, "b.txt", "b, resolved\n", f"fix: b (#2)\n\n(cherry picked from commit {original})")

    assert unmatched(repo, "master~3", "release", upstream="master") == []


def test_squashed_backport_passes_by_pr_reference(repo: Path) -> None:
    """A squash of the backport PR keeps the original subject with its PR number."""
    _commit(repo, "b.txt", "b, resolved\n", "[Backport release] fix: b (#9)\n\n* fix: b (#2)")

    assert unmatched(repo, "master~3", "release", upstream="master") == []


def test_direct_release_commit_fails(repo: Path) -> None:
    """A change made only on the release branch is reported."""
    _git(repo, "cherry-pick", "-x", _sha(repo, "(#1)"))
    direct = _commit(repo, "hotfix.txt", "x\n", "fix: release-only hotfix")

    offenders = unmatched(repo, "master~3", "release", upstream="master")

    assert [commit.sha for commit in offenders] == [direct]


def test_no_new_commits_passes(repo: Path) -> None:
    """A branch identical to its base has nothing to check."""
    assert unmatched(repo, "release", "release", upstream="master") == []


def test_missing_lists_unpicked_master_commits(repo: Path) -> None:
    """The report names master commits left out, however they were picked."""
    _git(repo, "cherry-pick", "-x", _sha(repo, "(#1)"))
    _commit(repo, "c.txt", "c, resolved\n", "[Backport release] fix: c (#9)\n\n* fix: c (#3)")

    assert [commit.subject for commit in missing(repo, "release", upstream="master")] == ["fix: b (#2)"]
