# CALIBRATION-RATE-MODELS-001: Direct curve adapters, quote domains and calibration entry selection

**Decision status:** Accepted 2026-11-15 — complete Hull–White entry, quote and fitted-state package.

**Implementation status:** Observed. Reconciliation to the accepted package is pending.

**Validation status:** Independent quote-clock and projection-role calculations verified. Production fitting, market references, restoration and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Accepted decision for explicit Hull-White calibration entry and quote domains

1. **Select the calibration profile explicitly.** Keep a dated
   contractual profile and a synthetic numeric profile. Dated
   requests use resolved exercise, fixing, accrual and payment
   schedules. Synthetic requests declare their frequency and
   retain numeric model-year support. Retire the semiannual
   default, schedule selection by omission and the dated
   frequency placeholder. Share checked Rust preparation where
   the profile inputs coincide.
   [Accepted dated adapter](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

2. **Bind the fit origin, market roles and clocks explicitly.**
   Use ACT/365F model time from the declared fit origin.
   Query each curve on its qualified native clock and rebase
   discounts to the fit origin with checked relative factors.
   Bind projection through the exact qualified index factor
   source or an expressly shared discount evaluator where the
   profile permits it. Retire implicit discount projection
   and a generic second discount curve as projection proxy.
   Propagate typed source failures. Preserve the separate
   market-volatility clock and coupon day counts.
   [Accepted model clocks and projection](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

3. **Use explicit ATM swaption quote types.** Admit positive
   finite Normal or unshifted Black annual volatility. Normal
   uses absolute rate units and admits signed forwards. Black
   uses relative volatility and requires a positive forward.
   Dated requests use the qualified European physical payer
   shared-curve profile and the computed par forward on a
   positive annuity. Verify an authored strike as an ATM
   assertion with an explicit decimal-rate tolerance. Synthetic
   ATM quotes elect that basis explicitly. Retire the Boolean
   quote-law field.
   [Accepted swaption preparation](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

4. **Use flat Normal cap and floor quotes.** Require literal cap
   or floor side, signed decimal strike and positive finite
   annual volatility. Dated requests retain contractual maturity
   and actual fixing dates on the qualified live term-index
   strip. Apply the elected first-coupon convention. Use the
   same strip and native quote clock for market premium
   conversion and implied-volatility inversion.
   [Accepted cap and floor schedules and inversion](02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

5. **Elect the fit mode and effective controls.** Support a free
   scalar κ and σ fit for swaptions or caps and floors, fixed-κ
   scalar sigma for caps and floors, and fixed-κ piecewise
   sigma for caps and floors. Require at least two eligible
   observations for two free parameters and one for fixed κ.
   Qualify identification separately. Require explicit controls
   or a selected versioned preset, a complete free-fit seed
   and an explicit piecewise sigma bracket. Reject inapplicable
   controls. Records 002–008 own schedule geometry, payoff
   methods and numerical search.

6. **Gate every quote in its native convention.** Preserve
   quote IDs and authored order. Reprice every selected quote
   on the returned fit and require a positive effective implied
   volatility tolerance in its native units. Preserve separate
   outcomes for solver termination, native fit and publication
   acceptance. Use tolerance ratios for mixed quote laws.
   Failed fits may retain diagnostics.
   [Accepted fit reporting](../calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

7. **Return complete fitted state under one Rust contract.** Bind
   κ and exactly one scalar sigma or schedule to its origin,
   clocks, roles, controls and actual fit evidence. Direct calls
   return the fit result. Dated market installation publishes
   the admitted block atomically. Preserve numeric knots and
   exact dated support in their original forms. Use the same
   checked construction, restoration and typed errors in Rust,
   Python and WASM.
   [Accepted fitted state](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

## Independent calculation checks

- A 180-day expiry is `.5` on ACT/360 and `36/73` on the
  ACT/365F model clock. At unit annuity and Normal sigma `.01`,
  using model time for the market quote gives a premium about
  0.6873% below the premium on the ACT/360 quote clock.
- For a benchmark accrual of `30/360`, projection endpoint
  factors `.9975` and `.995` give a simple forward of `6/199`.
  Using the discount endpoints `.995` and `.990` instead gives
  `2/33`. Rebasing the latter discount factors to the first
  endpoint gives the end discount factor `198/199`.

## Observed choice and rationale at the audit revision

The public curve-based HW functions adapt market curves to a common model clock and invoke explicit scalar or piecewise fits. They use synthetic schedules; the plan-driven engine supplies contractual schedules through separate internal entry points. Source-documented rationale is one curve-to-model adaptation shared by host callers. This is an observed contract, not approval of synthetic schedules as a substitute for market conventions.

## Observed scope and input domain

`SwaptionQuote` is ATM-only: finite positive expiry, tenor and volatility, with a Boolean Normal/Black convention and no strike field. `CapFloorQuote` requires finite positive maturity/volatility, finite strike of either sign, and Normal volatility; lognormal caps are rejected. Both wire types deny unknown fields and route deserialization through constructors; fit entry points revalidate publicly mutable quote fields. Normal volatility is an annual absolute rate volatility; Black volatility is annual relative volatility, both decimals.

`SwapFrequency` supports annual, semiannual and quarterly synthetic periods; semiannual is the default. Cap scalar config requires a finite positive quote-fit tolerance, with optional fixed κ and initial guess. Piecewise config requires fixed κ and explicit σ bounds. Configuration deserialization rejects unknown fields, but numerical config validation occurs when fitting.

## Observed algorithm and selection

1. The direct adapter chooses the discount curve's base date as valuation date, constructs `ModelDiscountCurve` for discount and optional projection, and normalizes both at that date. Each original base date/day count maps independently to ACT/365F model years.
2. Omitted projection uses the discount curve. Direct cap APIs accept a second `DiscountCurve` as a projection-factor curve, whereas the engine's distinct projection role is a `ForwardCurve`; these are different caller interfaces.
3. Curve lookup failures become NaN in the closures and are handled by the downstream pricing/solver validation. No replacement flat curve is installed.
4. Direct swaption fitting passes no schedule and uses the selected fixed-leg frequency. Direct cap fitting/bootstrapping likewise builds synthetic periods. The engine passes contractual schedules; its nominal Annual argument is ignored by schedule construction.
5. Swaption fitting always estimates scalar κ and σ and requires at least two quotes. Scalar cap fitting estimates both unless fixed κ selects the one-dimensional fit; one quote requires fixed κ. Piecewise cap fitting is an explicitly different function, with κ fixed.

The engine's quote selection, dated schedule construction, ATM strike gate, output keys and fit-based installation policy belong to the [market-adapter record](../calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication). The [explicit LMM helper](09-lmm-rebonato-calibration.md) owns LMM calibration.

## Observed conventions, limitations and failure behavior

Model time is ACT/365F independently of coupon accrual conventions. Direct APIs apply no settlement lag, calendar or business-day adjustment. The detailed [cap](02-cap-schedules-and-quote-inversion.md) and [swaption](03-atm-swaption-quotes.md) records define the resulting synthetic schedule. Curve adaptation does not establish that a second discount-factor curve reproduces a caller's intended projection convention.

A returned HW report can have `success=false` without the direct function returning an error. A valid fit tolerance governs quote acceptance, not numerical solver termination or model suitability. Parameters, clocks and report status must travel together; the direct API returns them without installing market data.

## Alternatives and tradeoffs

Contractual engine schedules retain calendars, stubs and payment/fixing lags but require convention data. Synthetic inputs provide a small numerical interface and expose frequency sensitivity. Automatic conversion of missing curves, shifted quotes or unsupported schedules is not an implemented alternative.

## Implementation and evidence

Owned sources: [curves.rs](../../../../finstack-quant/calibration/src/hull_white/curves.rs), [quotes.rs](../../../../finstack-quant/calibration/src/hull_white/quotes.rs), with entry validation in [cap_floor.rs](../../../../finstack-quant/calibration/src/hull_white/cap_floor.rs) and [swaption.rs](../../../../finstack-quant/calibration/src/hull_white/swaption.rs). The model-clock algorithm is separately documented [here](../models-analytic/rate-clock-and-projection.md).

Inline curve tests inspect direct/closure equality, omitted projection, differing curve clocks/base dates, one-quote rejection and serde defaults. [Engine regressions](../../../../finstack-quant/calibration/tests/calibration/hull_white_regressions.rs) compare equivalent dated curves across clock representations and enforce failed-fit handling. These are adaptation/invariant tests, not independent model-price benchmarks; none was executed.

An optional initial guess is an optimizer seed, not an approved production model or evidence of parameter identification.

## Follow-up tracking

Implementation reconciliation, adoption questions and missing evidence are tracked in the
[rate-model calibration follow-ups](../FOLLOW_UPS.md#calibration-hullwhite-and-lmm).
