"""Keep release branches a subset of master ("upstream first").

``check BASE HEAD`` fails when a commit in ``BASE..HEAD`` has no counterpart on
the upstream branch (``origin/master`` by default). A commit has a counterpart
when any of these holds:

- ``git cherry`` finds an upstream commit with the same patch;
- its message carries ``(cherry picked from commit <sha>)`` and ``<sha>`` is on
  upstream (``git cherry-pick -x``, which the backport bot uses, writes it, and
  the squash merge of the backport pull request keeps it);
- its message references ``(#N)`` and an upstream squash commit's subject ends
  with ``(#N)`` (the original pull request).

``missing HEAD`` prints, as a Markdown list, the upstream commits made after
``HEAD`` diverged that have no counterpart on ``HEAD``: what a release
candidate leaves out.

pr.yml runs ``check`` on pull requests into ``release/**``; release.yml runs it
before publishing a tag and uses ``missing`` for release-candidate notes.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import shutil
import subprocess
import sys

_CHERRY_PICKED = re.compile(r"\(cherry picked from commit ([0-9a-f]{7,40})\)")
_PR_REFERENCE = re.compile(r"\(#(\d+)\)")
_SEPARATOR = "\x1f"
_RECORD_END = "\x1e"


@dataclass(frozen=True)
class Commit:
    """One commit: full SHA, subject line, and full message."""

    sha: str
    subject: str
    message: str


def _run_git(repo: Path, *args: str, check: bool) -> subprocess.CompletedProcess[str]:
    git = shutil.which("git")
    if git is None:
        raise SystemExit("git is not on PATH")
    return subprocess.run(  # noqa: S603 -- resolved git executable with fixed subcommands; no shell.
        [git, *args], cwd=repo, check=check, capture_output=True, text=True
    )


def _git(repo: Path, *args: str) -> str:
    return _run_git(repo, *args, check=True).stdout


def _commits(repo: Path, *rev_args: str) -> list[Commit]:
    out = _git(repo, "log", "--reverse", f"--format=%H{_SEPARATOR}%s{_SEPARATOR}%B{_RECORD_END}", *rev_args)
    commits = []
    for raw in out.split(_RECORD_END):
        record = raw.strip("\n")
        if record:
            sha, subject, message = record.split(_SEPARATOR, 2)
            commits.append(Commit(sha, subject, message))
    return commits


def _cherry(repo: Path, upstream: str, head: str, limit: str) -> set[str]:
    """SHAs in ``limit..head`` with no patch-equivalent commit on ``upstream``."""
    lines = _git(repo, "cherry", upstream, head, limit).splitlines()
    return {line[2:] for line in lines if line.startswith("+ ")}


def _is_ancestor(repo: Path, sha: str, ref: str) -> bool:
    return _run_git(repo, "merge-base", "--is-ancestor", sha, ref, check=False).returncode == 0


def _pr_numbers(message: str) -> set[str]:
    return set(_PR_REFERENCE.findall(message))


def _upstream_prs(repo: Path, upstream: str, since: str) -> dict[str, str]:
    """Map each pull request number ending an upstream subject to its SHA."""
    prs = {}
    for commit in _commits(repo, f"{since}..{upstream}"):
        match = re.search(r"\(#(\d+)\)\s*$", commit.subject)
        if match:
            prs[match.group(1)] = commit.sha
    return prs


def unmatched(repo: Path, base: str, head: str, upstream: str = "origin/master") -> list[Commit]:
    """Return the commits in ``base..head`` with no counterpart on ``upstream``."""
    candidates = _cherry(repo, upstream, head, base)
    if not candidates:
        return []
    since = _git(repo, "merge-base", upstream, head).strip()
    upstream_prs = _upstream_prs(repo, upstream, since)
    result = []
    for commit in _commits(repo, f"{base}..{head}"):
        if commit.sha not in candidates:
            continue
        picked = _CHERRY_PICKED.findall(commit.message)
        if any(_is_ancestor(repo, sha, upstream) for sha in picked):
            continue
        if _pr_numbers(commit.message) & upstream_prs.keys():
            continue
        result.append(commit)
    return result


def missing(repo: Path, head: str, upstream: str = "origin/master") -> list[Commit]:
    """Return upstream commits after ``head`` diverged that ``head`` lacks."""
    since = _git(repo, "merge-base", upstream, head).strip()
    absent = _cherry(repo, head, upstream, since)
    if not absent:
        return []
    picked: set[str] = set()
    referenced: set[str] = set()
    for commit in _commits(repo, f"{since}..{head}"):
        picked.update(_CHERRY_PICKED.findall(commit.message))
        referenced |= _pr_numbers(commit.message)
    result = []
    for commit in _commits(repo, f"{since}..{upstream}"):
        if commit.sha not in absent:
            continue
        if any(commit.sha.startswith(sha) for sha in picked):
            continue
        match = re.search(r"\(#(\d+)\)\s*$", commit.subject)
        if match and match.group(1) in referenced:
            continue
        result.append(commit)
    return result


def main() -> int:
    """Run ``check`` or ``missing``; return the process exit code."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--upstream", default="origin/master", help="Branch every change must reach first")
    parser.add_argument("--repo", type=Path, default=Path.cwd(), help="Repository to inspect")
    commands = parser.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check", help="Fail on commits in BASE..HEAD that upstream lacks")
    check.add_argument("base")
    check.add_argument("head")
    report = commands.add_parser("missing", help="List upstream commits HEAD leaves out")
    report.add_argument("head")
    args = parser.parse_args()

    if args.command == "missing":
        for commit in missing(args.repo, args.head, args.upstream):
            print(f"- {commit.subject} ({commit.sha[:10]})")
        return 0

    offenders = unmatched(args.repo, args.base, args.head, args.upstream)
    if not offenders:
        print(f"Every commit in {args.base}..{args.head} is on {args.upstream}.")
        return 0
    print(f"Commits in {args.base}..{args.head} with no counterpart on {args.upstream}:", file=sys.stderr)
    for commit in offenders:
        print(f"  {commit.sha[:10]} {commit.subject}", file=sys.stderr)
    print(
        f"Land each change on {args.upstream.removeprefix('origin/')} first, then backport it "
        "(label the pull request `backport <release branch>`).",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
