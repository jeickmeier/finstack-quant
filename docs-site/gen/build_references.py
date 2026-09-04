"""Render the canonical bibliography with its exact reference anchors."""

import re

from common import REPO, SITE, markdown_heading_slug
from nbconvert.filters.markdown_mistune import markdown2html_mistune


def main() -> int:
    """Generate a local supporting bibliography without a duplicate reference source."""
    source = (REPO / "docs/REFERENCES.md").read_text()
    anchors = set(re.findall(r'<a\s+(?:id|name)=["\']([^"\']+)', source))

    def heading_anchor(match: re.Match) -> str:
        """Preserve the syllabus's Markdown heading slugs in nbconvert HTML."""
        slug = markdown_heading_slug(match[2])
        if slug in anchors:
            return match[0]
        anchors.add(slug)
        return f'<a id="{slug}"></a>\n\n{match[0]}'

    source = re.sub(r"^(#{1,6})\s+(.+)$", heading_anchor, source, flags=re.M)
    body = markdown2html_mistune(source)
    target = SITE / "public/references.html"
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(
        '<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Finstack Quant references</title><style>body{max-width:76ch;margin:3rem auto;padding:0 1.2rem;font:17px/1.65 system-ui;color:#183a32;background:#fafbf9}a{color:#286d60}h2{margin-top:3rem}code{font-size:.9em}</style><nav><a href="/">Analyst program</a></nav><main>'
        + body
        + "</main></html>"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
