# Duration-cell and sector hierarchical return attribution

- ID: PORTFOLIO-ATTRIBUTION-015
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

The grid method decomposes active return into curve-cell allocation, within-cell sector allocation and selection. Inputs are portfolio and benchmark position snapshots with cell label, sector label, decimal weight and return. Labels are grouping keys supplied by the caller; the method does not build duration cells itself. Each side is nonempty, all numerical inputs finite and weights sum to one within `1e-6`; signed weights are allowed.

## Ordered algorithm and fallback rules

1. Build the union of cell/sector groups in portfolio-first then benchmark first-appearance order. Aggregate weighted returns and net/gross weights at both cell and `(cell,sector)` levels.
2. Reject groups with positive gross weight and `|net weight|<=1e-6*gross weight`; otherwise divide weighted returns by net weight. This avoids defining subgroup returns for near-offset books.
3. Let `wP_c,wB_c` be cell weights and `rP_c,rB_c` cell returns. Curve allocation is `(wP_c-wB_c)*(rB_c-R_B)`.
4. Within a cell define sector fractions `zP_s=wP_s/wP_c`, `zB_s=wB_s/wB_c`. Sector allocation is `wP_c*(zP_s-zB_s)*(rB_s-rB_c)`; selection is `wP_s*(rP_s-rB_s)`.
5. If the whole cell is absent from benchmark, substitute its portfolio return for `rB_c`, and use the portfolio within-cell fractions and returns as benchmark comparators. Sector allocation and selection then vanish; the active contribution lands in curve allocation.
6. If just a sector is absent within an otherwise benchmark-present cell, use benchmark sector fraction zero and portfolio sector return as its comparator, so its selection vanishes while allocation carries the difference.
7. Aggregate three top-level effects with compensated sums; group construction uses ordinary accumulated amounts. Multi-period linking validates the three top-level effects and uses [Carino smoothing](17-carino-linking-and-reconciliation.md). It does not require identical detailed grids across periods and does not validate the caller-supplied detailed cell/sector rows in that linking path; the linked output contains aggregate effects and original periods.

## Alternatives and evidence

Using a fixed benchmark return for all absent cells, independent security selection, or full key-rate curve effects would produce different attributions. The actual fallback deliberately assigns out-of-benchmark return to the highest existing grid level. Near-zero-net rejection restricts signed-basket use even when total portfolio weights are valid.

[Source and inline identity/fallback tests](../../../../finstack-quant/portfolio/src/grid_attribution.rs) provide formula and local invariant evidence. [Dynkin–Hyman–Vankudre](../../../REFERENCES.md#dynkin-hyman-vankudre-1998) is the repository's hierarchy reference; no independently sourced return grid was rerun.

Arithmetic closure alone does not establish a valid duration or sector classification.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
