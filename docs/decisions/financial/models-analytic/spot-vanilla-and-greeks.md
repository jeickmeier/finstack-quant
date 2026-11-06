# MODELS-ANALYTIC-01: Spot Black–Scholes price, Greeks and calculation dispatch

**Decision status:** Draft (observed implementation, adoption pending).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Evidence inspected/located, not executed. Formula comparisons, round trips and invariants have different evidential strength; none establishes approval or universal accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Scope and input domain

European calls and puts with constant continuous carry. Checked spot and strike must be positive and finite, rates finite, expiry/volatility nonnegative for price; checked Greeks require strictly positive expiry and volatility.

## Choice and rationale

The kernel uses the Black–Scholes–Merton formula. Analytic derivatives avoid bump noise; this is an inferred benefit, not evidence of adoption. The public calculation request explicitly chooses BlackScholes, Black76 or Bachelier and either price or implied volatility.

## Algorithm

For T>0 and σ>0, d1=[ln(S/K)+(r−q+σ²/2)T]/(σ√T), d2=d1−σ√T; C=S exp(−qT)N(d1)−K exp(−rT)N(d2), with the analogous put. Expiry uses spot intrinsic; zero volatility uses intrinsic of discounted spot and strike. Finite prices are floored at zero. Delta is ±exp(−qT)N(±d1), gamma=exp(−qT)φ(d1)/(Sσ√T), vega=S exp(−qT)φ(d1)√T/100; theta combines diffusion and carry terms, then divides by configured days/year; rate/dividend rho divide analytic derivatives by 100. `option_calculation` first solves price or volatility, then computes Greeks at that volatility; it cannot return price alone when Greeks reject the domain.

## Selection and controls

`bs_price` and `bs_greeks` validate; `_unchecked` variants expose deterministic/expiry behavior without those checks. Theta defaults to core calendar days/year through calculation dispatch. Instrument selection is separately described in [ADR 0027](../valuations-equity-and-composition/0027-vanilla-option-model-selection.md). No exercise or discrete-dividend model is selected here.

## Conventions

Rates and σ are annual decimal inputs, time is a supplied year fraction, premium is currency per underlying unit. Spot delta/gamma are per spot unit; vega/rhos are per percentage-point change, theta per configured day. In contrast, the forward calculation returns undiscounted unit-vol Greeks even when Black76 price uses df.

## Limitations, failure and fallback

At expiry unchecked spot delta is zero exactly at the strike (strict >/<); raw Black/Bachelier boundary delta uses a different tie convention. `bs_price` permits zero vol/time but `option_calculation` fails when its mandatory checked spot Greeks cannot be computed. No stochastic carry, smiles, early exercise or cash-dividend scheduling occurs in this kernel.

## Alternatives and tradeoffs

Black76 and Bachelier are implemented request alternatives. Bump Greeks or smile-adjusted Greeks are possible but not selected here; analytic sensitivities hold the supplied volatility fixed.

## Implementation and validation evidence

[vanilla.rs](../../../../finstack-quant/models/src/closed_form/vanilla.rs), [black.rs](../../../../finstack-quant/models/src/closed_form/black.rs) and [calculation.rs](../../../../finstack-quant/models/src/closed_form/calculation.rs) own formulas, singular branches, units and dispatch. Inline `test_bs_greeks_call`, `test_bs_price_put_is_non_negative_for_deep_otm_case`, and `price_solve_matches_the_kernels` are formula/boundary/kernel-routing evidence. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) and [gen_vol_golden.py](../../../../finstack-quant/models/tests/golden/data/gen_vol_golden.py) provide separate QuantLib/mpmath price fixtures; they do not independently certify all Greeks. References: [Black–Scholes](../../../REFERENCES.md#black-scholes-1973), [Merton](../../../REFERENCES.md#merton-1973).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
