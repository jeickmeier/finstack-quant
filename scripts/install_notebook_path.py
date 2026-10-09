"""Expose the example notebook tree on the active interpreter's import path.

Writes ``finstack_notebooks.pth`` into site-packages so ``import _shared``
resolves from any working directory. The notebook runner still prepends its
selected tree on ``PYTHONPATH``, which outranks this path file.
"""

from __future__ import annotations

import argparse
from pathlib import Path
import sysconfig

PTH_NAME = "finstack_notebooks.pth"
REPO_ROOT = Path(__file__).resolve().parents[1]
NOTEBOOKS_ROOT = REPO_ROOT / "finstack-quant-py" / "examples" / "notebooks"


def notebooks_root() -> Path:
    """Return the example notebook directory that contains the ``_shared`` package."""
    root = NOTEBOOKS_ROOT.resolve()
    package = root / "_shared" / "__init__.py"
    if not package.is_file():
        raise FileNotFoundError(f"notebook shared package is missing at {package}")
    return root


def install_notebook_path(site_packages: Path) -> Path:
    """Write the notebooks root into ``site_packages`` as a path file.

    Parameters
    ----------
    site_packages:
        Directory Python scans for ``.pth`` files. Pass the active
        environment's purelib directory so every kernel using that
        interpreter can import ``_shared``.

    Returns:
    -------
    Path
        The path file that now points at the notebooks root.
    """
    site_packages.mkdir(parents=True, exist_ok=True)
    target = site_packages / PTH_NAME
    payload = f"{notebooks_root()}\n"
    if not target.is_file() or target.read_text(encoding="utf-8") != payload:
        target.write_text(payload, encoding="utf-8")
    return target


def main() -> None:
    """Install the notebooks path into this interpreter, or into ``--site-packages``."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--site-packages",
        type=Path,
        default=None,
        help="Directory that receives the path file. Defaults to this interpreter's purelib path.",
    )
    args = parser.parse_args()
    site_packages = args.site_packages if args.site_packages is not None else Path(sysconfig.get_path("purelib"))
    print(install_notebook_path(site_packages))


if __name__ == "__main__":
    main()
