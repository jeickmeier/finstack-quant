# VALUATIONS-EQUITY-AND-COMPOSITION-08: Selected-model Greeks, higher derivatives, Dividend01 and implied volatility

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-08`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

Selected-model risk reprices through the metric context for principal Greeks; native providers also expose analytical/tree Greeks. Implied volatility is a quote-equivalent inversion of the native Black/tree calculation, regardless of an alternative model used to produce a base PV.

## Scope and input domain

Live vanilla option risk under a selected registry model, optional active-volatility overrides and configurable bump controls. IV requires positive quantity and a finite nonnegative total premium; exercised/expired options are rejected.

## Algorithm

1. Selected delta/gamma/vega/vanna/volga delegate to generic finite-difference calculators preserving dispatch; selected rho uses a discount-only parallel rate bump. After an effective exercise observation, use lifecycle Greeks, with unused higher Greeks zero.
2. Selected theta uses one-day generic market roll, scaled by `365/days_per_year`. Exactly one calendar day before expiry it instead differences current PV against the previous date with a backwards-rolled market, avoiding a fabricated forward expiry quote.
3. Charm/color difference spot delta/gamma between today and tomorrow divided by the instrument day-count fraction, using unchanged market curves and selected repricing. They return zero if any exercise observation exists or fewer than two day fractions remain. Speed differences gamma at spot offsets ±h using prices at ±2h, ±h and base. Optional adaptive spot bump multiplies the configured bump by `min(1+2|S−K|/K,5)`.
4. Continuous Dividend01 bumps q by ±1 bp, floors down q at zero, normalizes actual width, and calls `instrument.value`. Discrete Dividend01 derives equivalent yield `q=−log(S*/S)/T`, rescales all future cash dividends to each bumped total PV and reprices through context.
5. IV divides premium by quantity, prices a `1e-8` volatility lower bound, rejects targets within `1e-10×max(|target|,1)` of that price, and doubles the upper bracket from 1 to at most 8. Brent uses `1e-12` root tolerance; trial price errors become NaN for the solver, and final premium residual must meet the same price tolerance.

## Selection, controls and conventions

ImpliedVol metric requires a quoted premium; no theoretical PV fallback is supplied. Native vanna uses four spot/vol reprices and volga a second vol difference in percentage-point units; active override bumps are capped at half the override volatility. All shared bump/dispatch owners are outside this slice.

## Limitations, failures and alternatives

Continuous Dividend01 bypasses selected dispatch while the discrete variant preserves it: alternative-model consistency is unresolved. Charm/color use static curves whereas selected theta rolls curves, so they are not derivatives of a single identical time scenario. IV may solve a different economic model from selected base pricing by design. Direct Heston parameter Greeks, alternative IV model inversion and central time derivatives are meaningful alternatives requiring agreement.

## Evidence

Source inline cases in [selected_model.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/selected_model.rs) and [dividend01.rs](../../../../finstack-quant/valuations/equity/src/dividend01.rs); [test_implied_vol.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_implied_vol.rs) and [test_greeks.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_greeks.rs) locate roundtrip and finite-difference comparisons. [Brent](../../../REFERENCES.md#brent-1973) provides the solver reference; roundtripping the same pricer is not independent price validation.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [mod.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/mod.rs)
- [selected_model.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/selected_model.rs)
- [spot_greek_decay.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/spot_greek_decay.rs)
- [dividend_risk.rs](../../../../finstack-quant/valuations/equity/src/equity_option/metrics/dividend_risk.rs)
- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
- [dividend01.rs](../../../../finstack-quant/valuations/equity/src/dividend01.rs)
