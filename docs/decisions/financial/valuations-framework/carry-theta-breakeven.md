# VALUATIONS-FRAMEWORK-13 — Horizon theta, market-held fixing carry decomposition, and breakeven solves

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in the source paths cited below.
- Validation status: Evidence inspected/located, not executed; no runtime checks or external accuracy comparison performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and conventions

Horizon defaults to one calendar day, may be overridden by a tenor, is unadjusted for business days, and is capped at the instrument's last payment date. A date at/before as-of gives zero. Outputs are **period totals**, with actual elapsed calendar days recorded separately. These algorithms distinguish rolling curves for theta from holding dated curves for carry decomposition; they are not alternative names for the same result.

## Theta and period cash

Reprice the base using instrument-signed economics. Project fixings crossed by the horizon from the original schedule, insert them in market history, then roll curves by elapsed days. Obtain product observation state implied by holding current spot, reprice that observed instrument at the rolled date, and return `cash_received + rolled_PV − base_PV`. Store cash carry, PV roll-down and realized horizon.

Cash classification includes cash coupons/fees/default-accrual/margin interest as income; principal, amortization, prepayment, revolving draws/repayments and recovery additionally count as economic cash. PIK, write-downs and collateral/IM/VM movements are excluded. Exclude negative issue-date notional draws identified by the instrument. Instruments excluding as-of cash use `(start,end]`; instruments including as-of cash use `[start,end)`, avoiding duplicate boundary cash across rolls. Convert each cashflow at its payment date into the base currency; cash is not discounted or reinvested in theta.

## Carry decomposition

Use held projected fixings but **unrolled dated curves** to price at the horizon. Income is cash income above; principal is total economic cash minus income. Default yield clock uses `(1+YTM)^τ` with context day count or Act/365F and an annual-frequency ICMA reference period where needed; yields at/below −100% fail, below −50% warn. Products may register a custom yield clock matching their quote convention.

For horizon accretion `A` and payment-date accretion `A_j`, compute reinvestment adjustment `Σ cash_j(A/A_j−1)`. Pull-to-par is `base_PV(A−1)−income−adjustment`. Roll-down is `horizon_PV−base_PV+principal−pull_to_par`. Funding cost is `base_PV(1/DF_repo(as_of,horizon)−1)` when the instrument names a repo curve, otherwise zero. Total carry is `income+pull_to_par+roll_down−funding`. If YTM is unavailable, pull-to-par becomes zero and roll-down absorbs the price move, with `CarryDecompositionDegenerate=1`. This fallback is reachable through normal registry resolution because a missing/inapplicable YTM dependency is skipped. A registered YTM calculator that fails still propagates its calculation error.

## Breakeven selection and solver

An explicit target/configuration is required. Linear mode returns `−carry_total/sensitivity`, rejecting absolute sensitivity below `1e−12`. Targets map ZSpread/OAS→CS01, YTM→DV01, ImpliedVol→Vega, BaseCorrelation→Correlation01, so reported changes follow those sensitivities' bp/vol-point/correlation-point units.

Iterative mode holds fixings and curves at the horizon and solves `carry_total+PV_bumped(horizon)−PV_base(horizon)=0` using fallible Brent, initial guess from the linear result, tolerance `1e−8` and 50 iterations. ZSpread/OAS shock the first discount curve; YTM shocks every unique declared discount/forward curve; implied vol shocks all present declared surfaces by `delta×0.01`. Base-correlation iteration is unsupported. ZSpread/OAS iteration is rejected when credit dependencies exist because quote-space CS01 would not match a discount-shift solve. Implied-vol iteration does not bump a scalar instrument vol override, even though Vega can. The named YTM target is implemented as curve shifts rather than an instrument yield-quote rebuild.

## Evidence, alternatives

[theta.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/theta.rs), [carry_decomposition.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/carry_decomposition.rs), [period_cash.rs](../../../../finstack-quant/valuations/core/src/metrics/period_cash.rs), [breakeven.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/breakeven.rs), and [breakeven inputs](../../../../finstack-quant/valuations/core/src/instruments/breakeven.rs) own these rules. Inline tests are behavioral/formula evidence, not executed. [Hull](../../../REFERENCES.md#hull-options-futures) provides carry context; the source also cites Tuckman–Serrat for constant-yield decomposition, but a dedicated canonical adoption reference for this precise split remains needed.

Alternatives such as financed cash reinvestment theta, business-day horizons and instrument-quote yield solves are not silently implemented here.

## Relationship to the accepted context lifecycle boundary

[CORE-MARKET-20's accepted package](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
requires explicit target-date context conditioning through family-owned source
semantics, atomic composites and active frozen-source/model qualification.
Horizon valuation and source knowledge are distinct; projected fixings or held-
spot product observations do not become actual publications. Cloning FX does
not imply held quotes. Reconcile actual theta producers and distinguish their
conditioning from unrolled-curve carry decomposition; context plumbing chooses
no universal carry/aging formula. Accepted record 20 does not adopt
this record's cash, observation, reinvestment, funding, breakeven or risk budgets.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
