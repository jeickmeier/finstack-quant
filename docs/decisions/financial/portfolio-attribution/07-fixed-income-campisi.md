# Campisi fixed-income carry, rates, spread and selection attribution

- ID: PORTFOLIO-ATTRIBUTION-007
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the stated owning source scope; discrepancies are named below.
- Validation status: Evidence inspected/located, not executed; independent accuracy qualification remains open.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and domain

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-attribution). This record does not
authorize a runtime change or certify the observed method.
