"""Reject broken local links, fragment targets and assets in the static export."""

from __future__ import annotations

import argparse
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit

from common import SITE


class Links(HTMLParser):
    """Collect links and anchors from rendered HTML rather than raw MDX."""

    def __init__(self) -> None:
        """Create empty link and anchor inventories."""
        super().__init__()
        self.links: set[str] = set()
        self.anchors: set[str] = set()

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        """Record supported URL-bearing elements and fragment targets."""
        fields = dict(attrs)
        if fields.get("id"):
            self.anchors.add(fields["id"])
        if tag == "a" and fields.get("name"):
            self.anchors.add(fields["name"])
        attribute = "href" if tag in {"a", "link"} else "src" if tag in {"img", "script", "iframe", "source"} else None
        if attribute and fields.get(attribute):
            self.links.add(fields[attribute])


def check(root: Path) -> list[str]:
    """Check every locally rendered destination and return actionable failures."""
    root = root.resolve()
    pages: dict[Path, Links] = {}
    for path in root.rglob("*.html"):
        parser = Links()
        parser.feed(path.read_text())
        pages[path] = parser
    if not pages:
        return [f"No rendered HTML found under {root}"]
    errors = []
    for page, parsed in pages.items():
        for href in parsed.links:
            url = urlsplit(href)
            if url.scheme or url.netloc:
                continue
            raw = unquote(url.path)
            target = (root / raw.lstrip("/") if raw.startswith("/") else page.parent / raw).resolve() if raw else page
            if not target.is_relative_to(root):
                errors.append(f"{page.relative_to(root)}: destination escapes export: {href}")
                continue
            if target.is_dir():
                target = target / "index.html"
            elif not target.exists() and not target.suffix:
                target = target.with_suffix(".html")
            if not target.is_file():
                errors.append(f"{page.relative_to(root)}: missing destination {href}")
            elif (
                url.fragment
                and target in pages
                and not {url.fragment, unquote(url.fragment)}.intersection(pages[target].anchors)
            ):
                errors.append(f"{page.relative_to(root)}: missing fragment {href}")
    return sorted(set(errors))


def main() -> int:
    """Validate the selected static export and report broken links."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=SITE / "out")
    args = parser.parse_args()
    errors = check(args.root)
    for error in errors:
        print(error)
    if not errors:
        print("All rendered local links and assets resolve")
    return int(bool(errors))


if __name__ == "__main__":
    raise SystemExit(main())
