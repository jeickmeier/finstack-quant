# PORTFOLIO-ATTRIBUTION-009: Duration-cell base returns and credit excess-return aggregation

- ID: PORTFOLIO-ATTRIBUTION-009
- Decision status: Accepted 2026-11-16 for checked duration-matched excess returns.
- Implementation status: Observed for the stated owning source scope. Runtime, caller and host reconciliation with the accepted contract are pending.
- Validation status: Independent cell-boundary, fill, dated-maturity, relative discount-price and signed aggregation examples were checked. Runtime, market-data and host qualification remain pending. The cited 1998 appendix was unavailable for verification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked duration-matched excess returns

1. Require one positive dated holding window, one reporting currency and compatible valuation, cash, fee and FX conventions. Use the same opening modified-duration convention on both sides, with finite nonnegative durations in years. Preserve unique reference-observation and position IDs and the selected reference source. Short positions carry negative weights.
2. Materialize finite strictly increasing bounds from the origin and explicit positive width, with at most 100,000 cells. Construct and query the same half-open intervals, closing only the last upper bound. The reference grid begins at zero and ends at the smallest materialized bound covering the longest observation. Identify cells by grid identity and ordinal. Rounded labels are display values. Check supplied tables for ascending nonoverlapping bounds. Gaps are valid and do not match a position.
3. Use widened equal-weight arithmetic means within observed reference cells. Fill leading empty cells from the first observed cell and interpolate interior gaps linearly by cell index. Stop at the last covered cell. Retain member IDs and counts for observed cells, source neighbors for fills, and distinct provenance for supplied and curve-modeled cells. Equal weighting and fill rules are choices for this operation. [Bloomberg duration-bucket description](https://data.bloomberglp.com/professional/sites/10/Bloomberg-Index-Publications-Fixed-Income-Index-Methodology.pdf).
4. Require an explicit lower duration bound and cell count for the synthetic grid. Convert each midpoint `m` to one maturity by adding `365*m` calendar days to the opening date, rounded to the nearest whole day with ties to even. Require the maturity to follow the closing date. Use qualified same-currency curve snapshots on the same reference basis. On each source's own clock and declared tail policy, obtain the checked dated relative discount factor from that endpoint to the same maturity. Compute `(D1-D0)/D0` from the admitted positive factors in widened arithmetic. Reject unrepresentable required returns and positive synthetic growth that would display as a return of `-1`. Label the result a midpoint zero-coupon proxy. [Accepted dated discount factors](../core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction).
5. Require exact signed decimal opening weights summing to one. Subtract each matched cell's admitted base return before weighting, then reconcile total, base and excess on the underlying values before rounding. Preserve position order and cell identity even when rounded side totals coincide. Rust owns checked construction and source-backed restoration across hosts. [Record 015](15-hierarchical-grid-attribution.md#accepted-decision-for-checked-duration-cell-and-sector-attribution) owns later mapping into hierarchical attribution.

At width `0.1`, the observed implementation assigns reference duration `0.9000000000000001` to a table ending at `0.9`, then rejects that duration on lookup. The accepted construction uses ten cells. An independent curve check gave `5%` from endpoint-relative prices `6/7` and `9/10` and `12.5%` from the unnormalized maturity factors.

## Observed choice and input domain at the audit revision

The excess-return module constructs a duration-matched reference return curve either from a reference bond universe or from start/end discount curves, then subtracts those base returns from portfolio returns. Duration is in years and returns are decimals. Cell width must be finite and positive; reference observations must be nonempty with finite nonnegative duration and finite return. It uses modified-duration matching, not key-rate, cashflow or stochastic credit matching.

## Reference-universe construction

Compute at least one cell and otherwise `ceil(max_duration/width)`, capped at 100,000. Place observations in grid-origin bounds `[i*width,(i+1)*width)` with a closed final upper endpoint; correct floating quotient indices against the exact cell bounds to avoid ULP gaps. Labels render bounds to one decimal place; collisions cause errors rather than silently merging cells.

Each observed cell's base return is the **unweighted arithmetic mean** of reference returns. The final cell is observed by construction. Fill leading empty cells with the first observed return and interior gaps with linear interpolation by cell index; no generic trailing extrapolation is needed. Record observation counts and filled/observed flags.

## Curve construction and excess calculation

The curve alternative uses each cell midpoint `m` and holding horizon `h`: require `m>h` in every cell, and positive finite discount factors, then set base return `DF_end(m-h)/DF_start(m)-1`. This is the holding return of a midpoint zero coupon as it rolls down. Each curve's own relative clock is used; the function does not check common currency or curve base-date alignment. It does not represent coupon income of a reference bond portfolio.

For a supplied base-return table, require unique nonempty labels, finite ordered bounds and returns, and ascending nonoverlapping cells. Gaps are allowed. Duration lookup is lower-inclusive/upper-exclusive except the last upper endpoint; unmatched duration errors. Validate portfolio returns/weights and a weight sum within `1e-6`; short weights are permitted. Compute each excess `r_i-base_i` before its weighted compensated accumulation, avoiding subtraction of two large aggregate returns. Return total, base and excess alongside cells/positions.

## Alternatives and evidence

Market-value-weighted reference averages, interpolation in duration values, key-rate matching and cashflow replication are alternatives with different economic meaning. The current cell average is equal-weighted and grid resolution is a caller choice.

[Source and inline boundary/interpolation/curve tests](../../../../finstack-quant/portfolio/src/excess_return.rs) provide local formula and numerical-edge evidence. [Dynkin–Hyman–Vankudre](../../../REFERENCES.md#dynkin-hyman-vankudre-1998) is the repository reference. No independent market-data benchmark ran.

The midpoint condition often implies `h<width/2`; it materially restricts curve-generated tables and does not establish arbitrary-horizon support.

## Follow-up tracking

Implementation and independent qualification are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
