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
        matches = {key: compact(expected[key]) in compact(raw)
                   for key in ("instrument", "market")}
        # Whole words catch numeric tokens split across lines or clipped. Only
        # visible table cells are expected; hidden original JSON cannot mask them.
        printed_words = {word["text"] for page in doc.pages for word in page.extract_words()}
        cashflow_matches = [
            all(word in printed_words for value in row if value for word in value.split())
            for row in expected["cashflowRows"]
        ]
        footer_matches = all(word in printed_words for word in expected["cashflowFooter"].split())
        assert cashflow_matches and all(cashflow_matches) and footer_matches, f"{name}: printed cashflow words or total missing"
        # Numbers in the visible measure tables must remain whole words. JSON
        # text is excluded so an intact raw dump cannot hide crushed columns.
        measure_words = {
            word["text"] for page in doc.pages
            for word in page.filter(lambda obj: obj["object_type"] == "char" and
                                    "IBMPlexSans" in obj.get("fontname", ""))
                            .extract_words()
        }
        measure_matches = {value: value in measure_words for value in expected["measureValues"]}
        if name.startswith("bond-"):
            assert measure_matches, f"{name}: bond report must contain supplied measures"
        assert all(measure_matches.values()), f"{name}: printed measure values wrap or are missing: {measure_matches}"
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
                        "measureWordMatches": measure_matches,
                        "cashflowRows": len(cashflow_matches), "cashflowCellsMatch": all(cashflow_matches),
                        "cashflowFooterMatches": footer_matches,
                        "outside": outside, "fonts": fonts, "maxUprightFontSize": max_upright_font_size, "images": images, "vectorPaths": vectors})
(root / "pdf-check.json").write_text(json.dumps(reports, indent=2) + "\n")
print(f"{len(reports)} reports passed text, font, vector and page-bound checks.")
