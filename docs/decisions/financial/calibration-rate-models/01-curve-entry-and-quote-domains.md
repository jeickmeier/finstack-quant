# CALIBRATION-RATE-MODELS-001: Direct curve adapters, quote domains and calibration entry selection

**Decision status:** Draft — observed implementation; adoption pending.

**Implementation status:** Observed for the source scope linked below; source/documentation discrepancies are stated explicitly.

**Validation status:** Evidence inspected/located, not executed. Internal round trips, analytic special cases and external provenance have distinct evidence roles.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory](README.md) and [source ledger](coverage.json).

## Choice and rationale

The public curve-based HW functions adapt market curves to a common model clock and invoke explicit scalar or piecewise fits. They use synthetic schedules; the plan-driven engine supplies contractual schedules through separate internal entry points. Source-documented rationale is one curve-to-model adaptation shared by host callers. This is an observed contract, not approval of synthetic schedules as a substitute for market conventions.

## Scope and input domain

`SwaptionQuote` is ATM-only: finite positive expiry, tenor and volatility, with a Boolean Normal/Black convention and no strike field. `CapFloorQuote` requires finite positive maturity/volatility, finite strike of either sign, and Normal volatility; lognormal caps are rejected. Both wire types deny unknown fields and route deserialization through constructors; fit entry points revalidate publicly mutable quote fields. Normal volatility is an annual absolute rate volatility; Black volatility is annual relative volatility, both decimals.

`SwapFrequency` supports annual, semiannual and quarterly synthetic periods; semiannual is the default. Cap scalar config requires a finite positive quote-fit tolerance, with optional fixed κ and initial guess. Piecewise config requires fixed κ and explicit σ bounds. Configuration deserialization rejects unknown fields, but numerical config validation occurs when fitting.

## Algorithm and selection

1. The direct adapter chooses the discount curve's base date as valuation date, constructs `ModelDiscountCurve` for discount and optional projection, and normalizes both at that date. Each original base date/day count maps independently to ACT/365F model years.
2. Omitted projection uses the discount curve. Direct cap APIs accept a second `DiscountCurve` as a projection-factor curve, whereas the engine's distinct projection role is a `ForwardCurve`; these are different caller interfaces.
3. Curve lookup failures become NaN in the closures and are handled by the downstream pricing/solver validation. No replacement flat curve is installed.
4. Direct swaption fitting passes no schedule and uses the selected fixed-leg frequency. Direct cap fitting/bootstrapping likewise builds synthetic periods. The engine passes contractual schedules; its nominal Annual argument is ignored by schedule construction.
5. Swaption fitting always estimates scalar κ and σ and requires at least two quotes. Scalar cap fitting estimates both unless fixed κ selects the one-dimensional fit; one quote requires fixed κ. Piecewise cap fitting is an explicitly different function, with κ fixed.

The engine's quote filtering, dated schedule construction, ATM strike gate, output keys and fit-failure installation policy belong to the [market-adapter record](../calibration-framework/18-hull-white-market-adapter.md). LMM calibration is the separate explicit [helper](09-lmm-rebonato-calibration.md), not a hidden HW or pricing fallback.

## Conventions, limitations and failure behavior

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
