"""Check the four Chromium reports; visual inspection remains a separate gate."""
import hashlib
import json
import pathlib
import re
import sys

import pdfplumber

root = pathlib.Path(sys.argv[1])
reports = []
for name in ("bond-A4", "bond-Letter", "xccy_swap-A4", "xccy_swap-Letter"):
    file = root / f"{name}.pdf"
    expected = json.loads((root / f"{name}-text.json").read_text())
    with pdfplumber.open(file) as doc:
        # Raw JSON uses Mono Regular. Extract it separately so rotated chart
        # labels and page-number footers cannot interrupt its reading order.
        raw = "".join(
            page.filter(lambda obj: obj["object_type"] == "char" and
                        "IBMPlexMono-Regular" in obj.get("fontname", ""))
            .extract_text() or "" for page in doc.pages
        )
        compact = lambda value: re.sub(r"\s+", "", value)
        matches = {key: compact(value) in compact(raw) for key, value in expected.items()}
        outside = [
            {"page": page.page_number, "text": char["text"]}
            for page in doc.pages for char in page.chars
            if char["text"].strip() and (char["x0"] < 10 or char["x1"] > page.width - 10
                                       or char["top"] < 10 or char["bottom"] > page.height - 10)
        ]
        sizes = [[page.width, page.height] for page in doc.pages]
        target = [595.28, 841.89] if name.endswith("A4") else [612, 792]
        assert all(abs(actual - wanted) < 1.1 for size in sizes for actual, wanted in zip(size, target))
        images = sum(len(page.images) for page in doc.pages)
        vectors = sum(len(page.curves) + len(page.rects) + len(page.lines) for page in doc.pages)
        fonts = sorted({char["fontname"] for page in doc.pages for char in page.chars})
        max_upright_font_size = max(char["size"] for page in doc.pages for char in page.chars if char.get("upright", True))
        assert max_upright_font_size <= 24, f"{name}: report text was enlarged by chart scaling ({max_upright_font_size:.2f}pt)"
        assert any("IBMPlexSans" in font for font in fonts)
        assert any("IBMPlexMono" in font for font in fonts)
        assert all(matches.values()) and not outside and images == 0 and vectors > 0, name
        reports.append({"file": file.name, "sha256": hashlib.sha256(file.read_bytes()).hexdigest(),
                        "pages": len(doc.pages), "pageSizes": sizes, "textMatches": matches,
                        "outside": outside, "fonts": fonts, "maxUprightFontSize": max_upright_font_size, "images": images, "vectorPaths": vectors})
(root / "pdf-check.json").write_text(json.dumps(reports, indent=2) + "\n")
print(f"{len(reports)} reports passed text, font, vector and page-bound checks.")
