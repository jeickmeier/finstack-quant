# PORTFOLIO-ATTRIBUTION-009: Duration-cell base returns and credit excess-return aggregation

- ID: PORTFOLIO-ATTRIBUTION-009
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and input domain

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
