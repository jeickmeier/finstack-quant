"""Tests for the code-context concatenation utility."""

from collections.abc import Sequence
import importlib.util
from pathlib import Path

import pytest

SCRIPT_PATH = Path(__file__).with_name("concat_code_context.py")
SPEC = importlib.util.spec_from_file_location("concat_code_context", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    msg = f"Could not load module spec for {SCRIPT_PATH}"
    raise RuntimeError(msg)
concat_code_context = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(concat_code_context)


def build_markdown(
    inputs: Sequence[Path],
    *,
    root: Path | None = None,
    max_file_bytes: int = 1_000_000,
) -> tuple[str, list[str]]:
    """Call the script helper with a typed test wrapper."""
    return concat_code_context.build_markdown(inputs, root=root, max_file_bytes=max_file_bytes)


def test_build_markdown_includes_code_files_and_excludes_artifacts(tmp_path: Path) -> None:
    """Code/text files are included while binaries and artifacts are skipped."""
    src = tmp_path / "src"
    src.mkdir()
    (src / "lib.rs").write_text("pub fn answer() -> i32 { 42 }\n", encoding="utf-8")
    (src / "README.md").write_text("# notes\n", encoding="utf-8")
    (src / "image.png").write_bytes(b"\x89PNG\r\n")
    target = src / "target"
    target.mkdir()
    (target / "generated.rs").write_text("pub fn generated() {}\n", encoding="utf-8")

    markdown, warnings = build_markdown([src], root=tmp_path)

    assert warnings == []
    assert "This is a code/text extract intended for LLM context." in markdown
    assert "## `src/README.md`" in markdown
    assert "## `src/lib.rs`" in markdown
    assert "image.png" not in markdown
    assert "generated.rs" not in markdown


def test_build_markdown_honors_max_file_bytes(tmp_path: Path) -> None:
    """Files larger than the configured byte limit are skipped."""
    src = tmp_path / "src"
    src.mkdir()
    (src / "small.py").write_text("print('ok')\n", encoding="utf-8")
    (src / "large.py").write_text("x = 'too large'\n", encoding="utf-8")

    markdown, warnings = build_markdown([src], root=tmp_path, max_file_bytes=12)

    assert "## `src/small.py`" in markdown
    assert "large.py" not in markdown
    assert warnings == ["Skipped src/large.py: file is larger than 12 bytes"]


def test_main_writes_default_output_under_output_root(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The default CLI output is written under the dedicated output root."""
    monkeypatch.chdir(tmp_path)
    src = tmp_path / "src"
    src.mkdir()
    (src / "lib.rs").write_text("pub fn answer() -> i32 { 42 }\n", encoding="utf-8")

    exit_code = concat_code_context.main(["src"])

    output = tmp_path / "code-context-output" / "code-context.md"
    assert exit_code == 0
    assert output.exists()
    assert "## `src/lib.rs`" in output.read_text(encoding="utf-8")
