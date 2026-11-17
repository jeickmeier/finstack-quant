# PORTFOLIO-ATTRIBUTION-007: Campisi fixed-income carry, rates, spread and residual attribution

- ID: PORTFOLIO-ATTRIBUTION-007
- Decision status: Accepted 2026-11-16 for checked single-period fixed-income attribution.
- Implementation status: Observed for the stated owning source scope. Runtime, caller and host reconciliation with the accepted contract are pending.
- Validation status: The primary formulation and independent exact coupon, shock, sector, signed-net and reconciliation examples were checked. Runtime, market-data and host qualification of the accepted contract are pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked fixed-income attribution

1. Require one dated, position-level request for a positive-length valuation window, one reporting currency, and compatible total-return, valuation, FX and fee bases. Require exact signed opening weights on each side summing to one. Use stable position IDs and an explicit ordered sector axis. Include cash and financing where applicable. Form each position's effects before aggregating the weighted effects.
2. Compute carry as opening annual cash coupon income divided by positive opening market value, multiplied by actual elapsed calendar days divided by 365. Replace generic annual yield and the independent `period_years` input. A zero-coupon position has zero coupon carry. This is a Campisi-style approximation with separately observed spread moves and sensitivities. [Campisi's original article](../../../REFERENCES.md#campisi-2000).
3. Compute Treasury effect as `-D_T*Δy` using the close-minus-open absolute decimal yield change for an opening-selected Treasury or government-curve tenor. Compute spread effect as `-D_S*Δz` using the absolute decimal change in qualified settlement Z-spreads on the same curve family, clock, compounding and claim or workout basis. Require opening normalized sensitivities for those quote experiments. Apply no time prorating to either price effect. Explicitly absent legs contribute zero. Missing applicable observations or incompatible sensitivities error. Zero and negative Z-spreads are valid. Keep one absolute-spread path. DTS belongs to spread-risk analysis. [Bond spread quotes](../valuations-bonds/12-spread-quotes.md#accepted-decision-for-checked-bond-spreads-and-price-inversion). [Bond spread duration](../valuations-bonds/16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk). [Ben Dor et al.](../../../REFERENCES.md#ben-dor-2007-dts).
4. Name the fourth component `residual` in the API and reports. Compute realized total return less carry, Treasury and spread. Residual includes effects the first-order proxies leave unexplained, including applicable roll, convexity, trading and FX effects in the admitted reporting return.
5. Use centered allocation `(W_P-W_B)*(r_B-R_B)` and the portfolio sector weight times each of the four component-rate differences. Portfolio-only sectors use whole-benchmark component comparators and have zero allocation. Benchmark-only sectors have allocation only. Distinguish observed absent-side rates from effective comparators. Reject sectors with gross exposure and exactly zero net weight. Remove the fixed `1e-6` net-to-gross threshold and admit nonzero signed net weights when the required outputs are representable.
6. Use bounded widened arithmetic, including sector quotients, to verify side components, centered sector totals and total active return before projecting each output once to binary64. Reject unrepresentable required outputs. Keep the caller-tolerance reconciliation helper as a display diagnostic and require a finite, nonnegative tolerance. Rust owns computation and source-backed restoration across hosts. [Record 017](17-carino-linking-and-reconciliation.md#accepted-decision-for-checked-carino-linking-and-reconciliation) owns Carino linking, cross-period validation and the positive-growth domain.

An independent 73-day example with opening market value `80`, annual coupon income `5`, Treasury duration `5` with a `+10 bp` move and spread duration `4` with a `+25 bp` move yielded `+125 bp` carry, `-50 bp` Treasury, `-100 bp` spread and `+225 bp` residual on a `+200 bp` realized return. A separate three-sector check with both one-sided cases reconciled the five effects to `+340 bp` active return.

## Observed choice and domain at the audit revision

For nonempty portfolio and benchmark snapshots, the implementation uses first-order fixed-income return approximations over a finite positive `period_years`. Each snapshot has sector, weight, total return, annual yield, modified duration, spread duration, treasury-yield change and spread change. Rates and changes are decimals; durations are years. Inputs, including spread level, must be finite. Spread level itself is unused in the calculation and need not be positive.

## Ordered calculation

1. For each holding calculate carry `y*h`, treasury `-D_mod*delta_y`, spread `-D_spread*delta_s`, and residual selection `total_return-carry-treasury-spread`. There is no separate convexity, roll-down, price/yield nonlinearity or transaction effect.
2. Require each side's weights sum to one within `1e-6`. Build sectors in portfolio first-appearance order, followed by benchmark-only sectors. Aggregate weighted component returns and group weights. Reject a nonzero-gross group whose absolute net weight is at most `1e-6` of its gross weight, avoiding unstable division in nearly offset long/short sectors.
3. Divide each sector's weighted amounts by its net weight. For a portfolio sector absent from the benchmark, use whole-benchmark component rates and total return as its comparator; its allocation is zero. Benchmark-only sectors retain their benchmark return in report fields.
4. Sector allocation is `(wP_i-wB_i)*(rB_i-R_B)`. Active carry, treasury, spread and selection are each `wP_i*(componentP_i-componentB_i)`. Their five totals reconcile algebraically to active return.
5. Optional multi-period linking applies the shared [Carino algorithm](17-carino-linking-and-reconciliation.md) and requires identical sector labels/order across periods. The report's reconciliation helper compares active return against the sum of top-level effects using caller tolerance; it does not independently validate that tolerance.

A separate DTS return mode is absent: for nonzero spread, `-(D*s)*(delta_s/s)` is algebraically `-D*delta_s`. This identity does not make DTS risk calibration equivalent to absolute-spread risk.

## Alternatives and evidence

Full repricing, key-rate curve decomposition, explicit roll/convexity terms and transaction-aware income calculations could explain parts of residual selection. The current residual is whatever the linear carry/rates/spread approximation leaves unexplained; it is not pure manager skill.

[Owning source and inline tests](../../../../finstack-quant/portfolio/src/fi_attribution.rs) give formula, grouping, signed-weight and linked reconciliation evidence. [Campisi](../../../REFERENCES.md#campisi-2000) and [DTS](../../../REFERENCES.md#ben-dor-2007-dts) are the repository references. Tests were inspected/located, not run; no independent position-level market-data benchmark is asserted.

## Numerical limits

Sector-level accumulation uses ordinary sums in places while overall totals use compensated sums; extreme cancellation is not qualified by the inspected evidence.

## Follow-up tracking

Implementation and independent validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution).
