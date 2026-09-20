# Final exact-set export QA — PASS

Rendered and visually inspected every page of the four PDFs in `publishing-complete/`, using bundled `pdftoppm -scale-to 1400 -png`. All 16 pages were inspected after the foreground-token accessibility fix and Table focus-wrapper integration. No source changes or test reruns were made.

| Report           | Pages | Chart page | Result |
| ---------------- | ----: | ---------: | ------ |
| bond A4          |     3 |          1 | PASS   |
| bond Letter      |     3 |          1 | PASS   |
| xccy swap A4     |     5 |          3 | PASS   |
| xccy swap Letter |     5 |          3 | PASS   |

The darker foreground preserves readable report text and chart labels. Chart titles, axes, tick labels and legends remain appropriately sized and unclipped. Stored-knot tables fit the page. Measure tables retain all rows; the Letter continuation repeats column headers. The Table focus wrapper causes no clipping, extra outline, or pagination regression in these exports.

No clipped report titles, table values or JSON; no completely blank pages. JSON wraps within page margins and continues across pages. Page counts and content breaks match the previously approved `publishing-final/` set: bond A4 has the existing sparse third page with the final two cashflow JSON lines, bond Letter continues measures and cashflows on page 3, and both cross-currency reports place charts/results on page 3 and finish cashflows on page 5. The sparse bond A4 ending is unchanged.

The existing automated `pdf-check.json` agrees with visual inspection: native instrument, market and cashflow text matches in all four reports; no out-of-bounds characters; no raster images; retained vector paths; maximum upright font size 18 pt.

Exact inspected PDF SHA-256 values:

- `bond-A4.pdf`: `833946cc836b085e0a65b8720e990188c947d7b83ceb6e37714a89d43ade657b`
- `bond-Letter.pdf`: `b6aee6bfceee6b5373a035692f127a06ab937a930183e0b84907d2fdfb263fb4`
- `xccy_swap-A4.pdf`: `937c02b5d26d6619fb84c079148fc222ec247b4368cdb27ea500323a62b6126e`
- `xccy_swap-Letter.pdf`: `facfb80a3543f22fab744ec8b05a04b53ff3d69ef7362134ab9ba10acd6b4bd0`
