# PORTFOLIO-ATTRIBUTION-015: Duration-cell and sector hierarchical return attribution

- ID: PORTFOLIO-ATTRIBUTION-015
- Decision status: Accepted 2026-11-16 for checked duration-cell and sector attribution.
- Implementation status: Observed for the stated owning source scope. Runtime, result restoration and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact calculations covered one-sided cells and sectors, occupied zero-net portfolio groups, very small signed benchmark nets and rounding of side returns. The original 1998 appendix was unavailable for verification. Runtime and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked duration-cell and sector attribution

1. Require one common dated positive-length holding window, reporting currency and compatible total-return, valuation, cash, fee and FX conventions. Admit complete disjoint member rows on each side with exact signed decimal opening weights summing to one, including applicable cash and financing. Use the same opening modified-duration convention and materialized grid on both sides. Identify cells by grid identity and ordinal and sectors by exact IDs from one declared taxonomy. Rounded labels are display values. Ambiguous or unclassified rows require an explicit governed bucket. [Duration grid identity and admission](09-duration-cell-excess-returns.md#accepted-decision-for-checked-duration-matched-excess-returns).
2. Distinguish zero gross weight from an occupied group whose signed weights offset. An occupied portfolio cell or sector can have exactly zero net weight and still contributes its weighted return. Its per-unit return is undefined. Every occupied benchmark cell and sector requires nonzero net weight to supply a comparator. Retire the relative `1e−6 × gross` cutoff. Admit small nonzero signed nets subject to checked arithmetic and result range.
3. For shared cells and sectors let `x` be cell net weight, `y` sector net weight, `q` weighted return contribution and `R_B` total benchmark return. Compute benchmark-centered duration-cell allocation `C = (xP − xB) × (rB_cell − R_B)`, within-cell sector allocation `A = (yP − xP × yB/xB) × (rB_sector − rB_cell)` and portfolio-weighted selection `S = qP_sector − yP × rB_sector`. Calculate observed benchmark group returns from the group's weighted contribution divided by its net weight. Use weighted contributions directly on the portfolio side.
4. Assign a portfolio-only cell's `qP_cell − xP × R_B` and a benchmark-only cell's `−qB_cell + xB × R_B` entirely to duration-cell allocation. In a shared cell, a portfolio-only sector contributes `qP_sector − yP × rB_cell` entirely to sector allocation. A benchmark-only sector uses the shared-cell allocation formula with zero portfolio sector weight. Selection is zero for one-sided sectors. Record observed benchmark returns and the applied one-sided treatment explicitly.
5. Use bounded widened arithmetic on exact opening weights and exact represented return values. Verify each cell's three effects sum to `qP_cell − qB_cell − (xP − xB) × R_B` and the full grid closes to `R_P − R_B` before rounding each output once. Reject unrepresentable required outputs. Preserve representable active return if displayed side totals coincide. Admit finite single-period arithmetic returns at or below `−100%`. [Single-period arithmetic basis](05-brinson-fachler.md#accepted-decision-for-checked-single-period-brinsonfachler-attribution).
6. Rust owns the admitted inputs, ordered grid, comparator provenance, effects and source-backed restoration. Existing Python and WASM paths project the checked result. Validate detail before linking. Pass the three validated effect totals and dated basis to the linker and retain each verified period's detail on its own grid. [Record 017](17-carino-linking-and-reconciliation.md#accepted-decision-for-checked-carino-linking-and-reconciliation) owns cross-period admission and reconciliation.

Independent exact calculations reconciled `+160 bp` active return to `+60 bp` duration-cell allocation, `+50 bp` sector allocation and `+50 bp` selection. In another case, an occupied portfolio cell with zero net weight contributed `+400 bp` of selection.

## Observed choice and domain

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
