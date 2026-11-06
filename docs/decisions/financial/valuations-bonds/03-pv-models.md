# VALUATIONS-BONDS-03: PV selection, quote pinning, discounting and fractional recovery of par

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and selection

Native model selection is joint `RatesCredit` when credit-factor inputs exist, or a credit curve accompanies exercise/rate-factor inputs; otherwise attached credit selects `HazardRate`, exercise selects `Tree`, and remaining bonds select `Discounting`. Explicit registry selection preserves its model family. Hazard-consuming models require an explicit hazard ID; Discounting/HazardRate reject exercise rights. `Tree` does not consume a hazard curve. Structural Merton is a distinct explicit registry model (record 08). [ADR 0028](0028-callable-bond-model-selection.md) documents selection rationale without adopting it.

Validation precedes quote handling. A finite quoted OAS prices inside the selected model. Other mutually exclusive price drivers convert to settlement dirty, then carry to `as_of`; a quoted PV therefore need not exercise the model's numerical kernel. Scenario spread shock is allowed only on Discounting without another non-Z price driver, and adds bp/10,000 to quoted Z-spread (default zero). Quote conversion computes all applicable quote metrics after converting the input to a common clean-price driver; applicable errors propagate, explicitly inapplicable fields and nonfinite returned metrics become `None`.

## Algorithms and conventions

**Discounting:** sum entitled signed holder flows `Σ CF_i D(as_of,T_i)`. For OAS `s`, at each cashflow derive continuous base zero rate `r=−ln D/t`, convert the configured quote-compounding spread to its continuous shift, and multiply by `exp(−shift·t)`. Time follows the discount curve. Neumaier compensated sums accumulate PV.

**Hazard FRP:** build dated discount/survival targets from `as_of` through the final adjusted cash-settlement date; include every calendar day, with `tree_steps` a minimum resolution (default one for this leaf). Add promised cash `CF_k D_k S_k exp(−s t_k)`. For each interval add `D_k S_k exp(−s t_k) R N_k W_k`, with `W_k=λ(1−exp(−(r+s+λ)Δt))/(r+s+λ)` through the shared stable FRP helper. `N_k` is after-event outstanding, replayed on actual repayment dates and including PIK/draws; recovery is clamped to [0,1]. No accrued-on-default recovery is added. OAS is continuous only; other quote compounding errors. No future cash gives zero. The same daily target helper is used by deterministic joint rollback.

## Limitations and alternatives

Risk-free cash discounting, survival-only discounting and FRP are different economic choices. Only the first and FRP paths are present here; market-value recovery would be a new model. The FRP exact interval weight is exact under its interval-constant inputs, not a guarantee for curved within-day rates or stochastic dependence. Quotes can hide missing calibration until a risk metric is requested. The quoted-price and model-value distinction must remain visible in adoption criteria.

## Evidence

Owning code: [native dispatch](../../../../finstack-quant/valuations/fixed-income/src/bond/types/pricing.rs), [native lifecycle](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs), [family registration](../../../../finstack-quant/valuations/fixed-income/src/family.rs), [registry envelope/MC diagnostics](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/mod.rs), [discount kernel](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/discount.rs), [hazard kernel](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/hazard.rs), [quote orchestration](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/compute.rs), [price metrics](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/prices.rs). Hazard inline tests compare zero hazard with discounting, require explicit curves and reject optionality; these are limiting-case/internal consistency evidence. [Callable credit-risk tests](../../../../finstack-quant/valuations/tests/instruments/bond/metrics/callable_credit_risk.rs) locate model-sensitive risk comparisons, not independent callable-credit accuracy qualification. [Quote tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/tests.rs) inspect applicable-error propagation and nonfinite-field handling. Reference: [Hull–White credit valuation](../../../REFERENCES.md#hull-white-2000-cds).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
