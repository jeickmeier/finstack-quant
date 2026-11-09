# CORE-MARKET-22: Curve, inflation, FX, and scalar market-change measurement

- Decision status: **Draft — observed implementation, adoption pending.**
- Implementation status: **Observed** in the owning paths identified below; discrepancies are stated explicitly.
- Validation status: **Evidence inspected or located, not executed.** Formula checks, invariants, and round trips are distinguished below; no production accuracy certification is implied.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Choice and rationale

The diff helpers reduce market snapshots to pointwise or unweighted average shifts with type-dependent units. These are measurement conventions for attribution/scenarios, not full repricing or risk-weighted curve changes. They can discard nonparallel structure.

## Algorithm and actual selection

Default Standard tenors are `[0.25,0.5,1,2,3,5,7,10,30]` years. Dynamic uses baseline curve knots only; Custom accepts a caller list. Lists must be nonempty and finite, but need not be sorted, unique or positive. Average measurement skips nonpositive times and errors if none remain. It evaluates both snapshots at the **same numeric tenor** on each curve's own base/clock, checks finite values, zeros differences within `1e-14*max(abs(v0),abs(v1),1)`, and averages all remaining samples equally. Duplicate tenors therefore alter weights. Per-tenor helpers validate finiteness but neither skip nonpositive times nor apply the same equality tolerance.

- Discount shift: continuous zero-rate difference times 10,000, in bp.
- Hazard shift: difference of each curve's `cds_quote_bp` using its own stored interpolation selection, in bp. That can compare quoted spreads with the local-hazard fallback after a synthetic bump.
- Credit dispatcher: baseline Hazard selects spread measurement; baseline Discount selects continuous-zero measurement; other types error. The comparison snapshot must supply the same required type.
- Inflation-curve shift: on the fixed Standard grid, annual zero inflation `(CPI(t)/baseCPI)^(1/t)-1`, difference times 10,000.
- Inflation-index shift: latest **raw observation** from each snapshot, `(level1/level0-1)*10000`; both must be positive finite. Dates can differ. Lag, seasonality, interpolation and publication eligibility are deliberately bypassed.
- Inflation source dispatcher prefers projected curves when present in both snapshots; otherwise selects observed indices when present in both. When neither paired mode exists, it returns the curve-path error.
- FX shift: date-specific default-policy FX queries in each snapshot, `(rate1/rate0-1)*100`, in percent.
- Scalar relative shift: same formula in percent, rejects baseline absolute value below 1e-15 and nonfinite result. Absolute shift is `value1-value0` in native units. Price comparisons require equal currencies; mixing Money and Unitless errors. Negative finite baselines remain allowed.

## Domain, limitations, and alternatives

These functions do not align calendar maturities when curve bases differ, align quote pillars, adjust currency mismatches, match inflation print dates, or produce value-weighted changes. Average subtraction/accumulation uses ordinary f64 summation with finite checks. Inflation-index and FX ratio results have no additional explicit post-ratio finiteness guard in these functions; their positive finite input domain can still admit extreme ratios.

A common-date maturity grid, union/intersection of quote nodes, signed absolute price changes near zero, or risk-weighted shifts are meaningful alternatives; Custom/per-tenor measurements already provide some explicit control.

## Evidence

Owning implementation and inline tests: [diff.rs](../../../../finstack-quant/core/src/market_data/diff.rs). Located tests explicitly cover raw inflation prints bypassing seasonality/lag, new publication dates, hybrid source preference, credit type dispatch, incompatible scalar units, large-magnitude equality and invalid grids. These are calculation-contract tests; no independent attribution accuracy benchmark is asserted. [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income) provides fixed-income risk context, while the exact grid and aggregation choices are repository policies. [Hazard quote-risk](hazard-quotes-and-risk.md) documents why spread shifts are not automatically recalibrated CDS spreads.

## Relationship to the accepted CPI measurement boundary

[CORE-MARKET-10's complete accepted package](inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks)
requires checked annual growth and retains the original CPI source/anchor after
valuation reference advancement. Reconcile duplicated raw-ratio CAGR arithmetic
and source error handling. Comparing own-origin numeric tenors does not by
itself align calendar maturities or actual observed reference periods. This
record's measurement/grid/aggregation choices remain Draft. Record10 was
accepted on **2026-11-08** with reconciliation/qualification pending; its
source contract does not select an attribution convention.

## Relationship to the accepted monthly comparison boundary

[CORE-MARKET-11's complete accepted package](inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability)
selects raw exact monthly observations with version/rebasing/seasonal identity
and explicit knowledge cutoff. A latest stored level from each dataset may
refer to different months or vintages; their relative difference is not
automatically a comparable fixing change. Reconcile actual source selection and
error propagation without treating this record's observed latest-print proxy
as adopted. Record11 was accepted on **2026-11-08**, with reconciliation/
qualification pending; these remaining measurement choices stay Draft.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in the
[core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
