# Equity Heston Fourier and Monte Carlo adapters

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-10`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The Heston adapters use caller-provided market parameter scalars; no equity-option calibration is performed here. Fourier and QE Monte Carlo are two explicit implementations of the same European stochastic-variance model with different numerical controls.

## Scope and input domain

Live European equity options without future discrete dividends. Required unitless Heston parameters are resolved by the strict shared market helper; lifecycle values resolve first. Ordinary spot/rate/q and volatility-resolution inputs are still collected, even when the option surface volatility is not used as the Heston variance parameter.

## Algorithm

Fourier: collect expiry-equivalent r/q and spot, resolve strict Heston parameters, invoke shared `heston_price` with no adapter-specific integration config, then multiply the unit price by signed quantity.

MC: construct `HestonProcess` and `QeHeston`, use a uniform grid with `max(round(T×steps_per_year),10)` steps, initialize `[S,v0]`, simulate a European terminal intrinsic payoff with quantity included and apply `exp(−rT)` discounting. Use Philox seeded by model configuration/instrument identity. Return mean PV; attach `mc_stderr` only when positive.

## Selection, controls and conventions

Registry keys `HestonFourier` and `MonteCarloHeston` are explicit alternatives. Path count accepts `model_config.mc_paths`; the constructor uses 100,000 paths and 252 steps/year before overrides. The generic engine configuration controls parallel execution; no adapter-specific antithetic, confidence target or calibration override is applied here.

## Limitations, failures and alternatives

A full volatility surface is not fitted by selecting Heston. Flat expiry-equivalent rates and q do not reproduce pathwise term structures. Standard error measures sampling uncertainty, not QE bias or parameter error; shared mathematical parameter validation is not a market-fit certificate. Fourier offers a deterministic alternative; default integration and MC budgets do not establish an accuracy bound.

## Evidence

[Heston](../../../REFERENCES.md#heston-1993), [Andersen QE](../../../REFERENCES.md#andersen-2008-heston-qe); [test_alt_models.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_alt_models.rs) and inline [heston_mc_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/heston_mc_pricer.rs) locate cross-engine and input-failure cases. The Heston-MC limiting-case comparison in `test_alt_models.rs` is slow/ignored in the ordinary suite, and some comparisons reuse library closed forms. Underlying process/transform algorithms are owned by the models audit; no parameter calibration or new simulation was run. Cross-engine agreement is not a complete independent certification.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [heston_mc_pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_option/heston_mc_pricer.rs)
- [mod.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/mod.rs)
