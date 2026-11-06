# Equity futures carry, quanto adjustment and risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-03`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The fair futures quote is a carry projection to settlement, optionally multiplied by a fixed-currency quanto drift adjustment. Position PV uses shared listed-future mark-to-market semantics rather than discounted terminal cashflow valuation.

## Scope and input domain

Positive finite spot/forward, positive finite cash dividends no later than settlement, same-currency futures or a fully specified cross-currency quanto. The equity and FX surfaces used for quanto must be Black-lognormal strike surfaces.

## Algorithm

1. Validate listed terms and currency relationships. Select the asset discount curve for quanto, otherwise the configured discount curve; use ACT/365F to settlement.
2. Without a cash-dividend vector, `Fdom = S exp(-qT)/Dasset`; otherwise `Fdom=(S−Σ Di Dasset(ti))/Dasset(T)` using future dividends only and ignoring continuous yield.
3. For quanto, sample equity volatility at `(T,Fdom)`. Resolve unitless FX spot as settlement currency per underlying currency, derive `FXforward=FXspot Dasset/Dsettlement`, and sample its Black surface there. Compute `Fquanto=Fdom exp(−ρ σequity σFX T)`.
4. Mark selection and `contracts × multiplier × position_sign × (mark−entry)` delegate to `ListedFutureTerms`, including quote/settlement states. No residual cashflows are emitted.
5. Spot delta is point delta times domestic carry derivative times `Fquanto/Fdom`; it holds sampled quanto volatilities fixed. Basis uses embedded quoted price when present, otherwise the model fair quote, minus current spot; the FuturesPrice metric uses resolved mark.

## Selection, controls and conventions

Quanto must be absent when underlying and settlement currencies match and present otherwise, with matching asset currency and equity surface ID. Surface lookups clamp via the shared surface helper. Rate risk is shared combined/key-rate repricing. The direct fair-price/delta helpers continue to calculate a model projection even if PV uses a fixed embedded quote.

## Limitations, failures and alternatives

There is no stochastic-rate futures/forward convexity adjustment. Analytic delta omits changes in smile samples as spot shifts the ATM/FX-forward sampling point. A quote-marked or already-settled PV can therefore have different bumped risk from the reported model delta/basis. Model-consistent finite differences or explicit quote-risk reporting are meaningful alternatives, not established approvals.

## Evidence

[Hull futures carry](../../../REFERENCES.md#hull-options-futures) and [quanto reference](../../../REFERENCES.md#wystup-fx-options); inline tests in [types.rs](../../../../finstack-quant/valuations/equity/src/equity_future/types.rs) and the [equity-future regression fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/regression_goldens/equity_future/spx_es_3m.json) locate carry/quote/quanto evidence. The fixture is a regression reference; independent exchange reconciliation and convergence evidence are not established.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_future/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/equity_future/metrics.rs)
