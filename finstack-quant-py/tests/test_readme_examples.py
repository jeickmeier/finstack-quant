"""Run the Python code samples in the repository and package READMEs.

The quick starts are the first code a new user copies, so each fenced
```` ```python ```` block must run against the built extension. A block whose
fence carries `no-run` (```` ```python no-run ````) is skipped; use it only for
a sample that shows what not to write.
"""

from __future__ import annotations

from pathlib import Path
import re

import pytest

# Run by `mise run python-test --doctest`, with the stub doctests.
pytestmark = pytest.mark.doctest

ROOT = Path(__file__).resolve().parents[2]
READMES = (ROOT / "README.md", ROOT / "finstack-quant-py" / "README.md")
FENCE = re.compile(r"^```python(?P<flags>[^\n]*)\n(?P<code>.*?)^```$", re.M | re.S)


def _blocks() -> list[tuple[str, str]]:
    blocks = []
    for readme in READMES:
        text = readme.read_text(encoding="utf-8")
        for match in FENCE.finditer(text):
            if "no-run" in match.group("flags").split():
                continue
            line = text.count("\n", 0, match.start()) + 1
            blocks.append((f"{readme.relative_to(ROOT)}:{line}", match.group("code")))
    return blocks


_BLOCKS = _blocks()


@pytest.mark.parametrize("code", [code for _, code in _BLOCKS], ids=[where for where, _ in _BLOCKS])
def test_readme_python_block_runs(code: str) -> None:
    """One README sample executes in a fresh namespace."""
    exec(compile(code, "<readme>", "exec"), {"__name__": "__readme__"})
