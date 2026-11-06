# PORTFOLIO-EXECUTION-003 — Metric applicability, canonical pricing and best-effort fallback

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

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

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
