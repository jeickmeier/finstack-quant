# PORTFOLIO-EXECUTION-003 — Metric applicability, canonical pricing and best-effort fallback

- **Decision status:** Accepted 2026-11-15 — complete valuation profile, canonical PV, atomic metric enrichment and failure coverage package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent menu, failure-policy and signed scaling examples verified. Production pricing, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio valuation profiles and failures

1. **Require an explicit metric menu and risk policy.** Use one
   ordered list of exact metric IDs with stable first-occurrence
   deduplication. An empty list requests canonical PV only.
   `Standard` is an opt-in preset for `[dv01]`. Retire implicit
   Standard selection and use the same list for additional
   metrics. Resolve exact IDs through the selected registry.
   Reject unknown IDs before calculations, including for an
   empty portfolio.
   [Metric orchestration](../valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

2. **Resolve structural applicability on the selected product basis.**
   Report structurally inapplicable requests separately from
   computed zero and failed calculations. A failed required child
   or missing required input remains a failure.
   [Accepted valuation lifecycle and composite basis](../valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

3. **Use a single canonical prepared valuation.** Bind the immutable
   portfolio and market, selected model and registry, effective
   settings, provider and dates through the shared executor.
   Admit base PV and its required details before risk enrichment.
   Record portfolio valuation cut and product-effective pricing
   dates. The ordinary entry uses the portfolio date. The explicit
   date entry prices a separate request. Retire the
   `Instrument::value` retry.
   [Shared executor](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope).

4. **Limit best effort to qualified risk-only failure.** Strict mode
   aborts when an applicable requested metric fails. Best-effort
   risk mode can retain the same qualified canonical PV and
   base-owned outputs when the pricer identifies an isolated metric
   failure after PV admission. Withhold that failed enrichment
   batch, including values computed before failure. Base pricing,
   required validation, sizing, FX and NAV assembly errors abort
   under either policy.
   [Atomic metric publication](../valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

5. **Report PV and risk coverage separately.** Keep ordered requested
   outcomes for published, structurally inapplicable, failed and
   batch-withheld metrics. Preserve the failed metric or dependency
   and typed cause. Record when no risk was requested. A qualified
   PV can feed book rollup; risk consumers use the actual metric
   coverage.
   [Book valuation pairing](02-book-hierarchy-and-rollups.md#accepted-decision-for-checked-book-hierarchy-and-nav-rollups).

6. **Scale the native mark once, then translate once.** Keep the
   instrument result unscaled. Apply the signed holding multiplier
   to native Money and translate once at the selected portfolio
   cut under the reporting FX policy. Applicable qualified marks
   may survive a risk-only failure. Missing required FX fails.
   Records 008 and 010 own FX and additive risk aggregation.
   [Holding scale](01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing) ·
   [Reporting FX](08-spot-and-forward-fx.md) ·
   [Additive metrics](10-metric-aggregation-and-reporting.md).

7. **Use one Rust contract across hosts and reuse.** Rust owns
   profile admission, exact Money, ordered outcomes and typed
   errors across Python and WASM. Reuse requires compatible
   inputs, profile and provenance. A degraded run cannot satisfy
   a strict request. Records 004 and 005 own scheduling and
   selective reuse.
   [Execution record 004](04-execution-order-and-reduction.md) ·
   [Reuse record 005](05-dependencies-and-selective-reuse.md).

## Independent calculation and policy checks

- For a two-deposit composite, `[dv01, ytm, dv01]` resolves
  to `[dv01, ytm]`. DV01 is applicable and YTM is
  structurally inapplicable.
- An admitted USD 200 mark at a signed `−50%` holding becomes
  USD `−100`; a qualified rate of 0.90 EUR per USD translates
  it to EUR `−90`. A risk-only enrichment failure in best-effort
  mode can retain both marks. Required FX failure aborts either
  mode.
- A missing input in canonical PV preparation fails both modes.
  A confirmed metric-only lookup failure after qualified PV fails
  strict mode; best effort retains the admitted mark and withholds
  the failed metric batch.

## Observed choice and rationale at the audit revision

Ordinary portfolio valuation requests canonical instrument pricing with an exact metric menu, then applies the holding scale and reporting FX. PV-only is an explicit empty menu, not the absence of a pricing operation. The distinction between structurally inapplicable metrics and failed requested metrics prevents an unsupported risk from being represented as a successful zero. Shared ownership is already required by [accepted ADR 0022](../../architecture/0022-portfolio-evaluation-state.md).

[Accepted ADR 0046](../../architecture/0046-portfolio-execution-operational-contract.md) adopts the exact-menu, explicit applicability and strict-risk distinctions. It does not adopt every observed metric default, fallback dispatch or instrument applicability declaration documented below.

## Algorithm and controls

`PortfolioValuationOptions` defaults to `strict_risk=true`, `RequestedMetrics::Standard`. Standard currently means DV01; PV is always produced. `StandardPlus` appends first-occurrence unique metrics, and `Only` stably deduplicates exactly the caller's menu. `Only([])` becomes the distinct PV-only profile. String-name construction resolves names against the standard registry rather than accepting arbitrary permissive metric text.

Each position is offered the menu through `Instrument::applicable_metrics`. The selected metric registry is the per-state option registry, otherwise `STANDARD`; composite instruments own their supported additive-leg union. Removed metrics are retained in `inapplicable_metrics` and do not make the position degraded.

1. Call `price_with_metrics` with the applicable menu, effective market-state date, and state-owned pricing options. Options include caller config and a calibration recalibration provider.
2. On success retain the complete unscaled `ValuationResult` and mark requested risk complete.
3. Under strict policy, any canonical pricing error becomes a position-context error preserving its core kind, including a PV-only canonical error.
4. Under best effort, **any** canonical error attempts `instrument.value(market, date)`. If this fails too, return both path messages with the fallback error kind. Otherwise create a stamped PV-only result using caller metadata and store the original canonical failure in `risk_error`.
5. A fallback with a nonempty applicable metric request sets `risk_metrics_complete=false`; with no applicable metrics it sets true but still retains `risk_error`. Therefore the portfolio degraded list, which selects incomplete risk, can be empty despite a PV-only canonical-path failure.
6. Scale the native PV by the position multiplier, [spot convert](08-spot-and-forward-fx.md), and retain `metric_scale` for later additive risk scaling. Stored measures remain instrument-level until aggregation.

`value_portfolio` uses the portfolio date; `value_portfolio_at` uses its explicit date without changing the portfolio. Best-effort fallback calls the instrument's value interface rather than retrying the same canonical options/provider path. Stamping the caller config does not by itself prove that both paths use identical selected-model behavior.

## Limitations and alternatives

Fallback is not limited to a missing metric error; numerical, market-data or other canonical errors may trigger it. A narrower error policy would be a substantive behavioral change requiring evidence and agreement. Returning a per-position error without a PV is an alternative, not implemented here. Strict versus best-effort identity must remain part of reuse under accepted ADR 0022; best effort cannot satisfy strict requests.

The exact applicability list depends on product/provider registration. This record does not certify every instrument's classification or justify generic zero risks. Native portfolio validation boundaries are documented in [01](01-position-sizing-and-validation.md). Complete host-language adapters are outside this source slice.

## Evidence

Owning sources are [valuation.rs](../../../../finstack-quant/portfolio/src/valuation.rs), [executor.rs](../../../../finstack-quant/portfolio/src/evaluation/executor.rs), [plan.rs](../../../../finstack-quant/portfolio/src/evaluation/plan.rs), [state.rs](../../../../finstack-quant/portfolio/src/evaluation/state.rs) and [error.rs](../../../../finstack-quant/portfolio/src/error.rs). Inspected [fallback tests](../../../../finstack-quant/portfolio/tests/valuation_fallback.rs) distinguish strict PV-only, retained canonical details, degraded risk and caller metadata. [Evaluation-plan tests](../../../../finstack-quant/portfolio/tests/evaluation_plan.rs) locate per-position narrowing and explicit empty-menu contracts. These use behavioral probes; they are not numerical model accuracy evidence.

## Follow-up tracking

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
