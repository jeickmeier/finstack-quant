# Rate futures projection, convexity and option rate risk

- ID: VALUATIONS-RATES-10-RATE-FUTURES
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Rate futures project an exchange reference rate, add an explicitly supplied or calculated futures/forward convexity adjustment, and map to `100*(1-rate-adjustment)`. Term simple rates, daily arithmetic averages and compounded-in-arrears with zero lookback are supported; lockout/shift/lookback alternatives are rejected. Daily settlement and mark selection are shared [listed-product methodology](../valuations-framework/listed-products.md), not bond-style discounted principal valuation.

## Algorithm and controls

1. Resolve fixing from the explicit field or last trading date; absent reference start is fixing plus two **calendar** days, and absent end adds `delivery_months`. Validate ordered dates. Term fixing cannot follow period start; overnight final fixing must lie within the reference period.
2. A term fixing strictly before valuation requires an exact historical observation and has zero forward sensitivity. Otherwise project the period's simple forward and set parallel sensitivity to one. Daily arithmetic/compounded rates delegate to the shared overnight projector, retaining already fixed observations and calculated remaining-forward sensitivity. Explicit `index_id` chooses fixings; otherwise the forward curve ID is reused.
3. Fully fixed projections have zero convexity. An explicit decimal adjustment is multiplied by remaining parallel-forward sensitivity.
4. Without explicit adjustment, a term future requires a Normal, strike-axis volatility surface, sampled at forward-curve time to start and projected rate. Vol must be in `[0,0.05]` decimal absolute rate units. Add `0.5*sigma^2*T_start*T_end`, scaled by remaining sensitivity. This is the implemented approximation, not a Black-vol formula.
5. Automatic overnight convexity requires explicit constant Hull–White mean reversion and sigma, rejecting sigma schedules and substitution of a surface for model parameters. For each unfixed daily observation use ACT/365F times, bond loading `b=B(t,u)`, state variance and covariance. Accumulate marginal exponent `b*0.5*sigma^2*B(0,t)^2 + b^2*Var[x_t]`, plus pairwise `b_i*b_j*Cov[x_i,x_j]`. Compounded adjustment is `(1+projected_rate*tau)*expm1(total_exponent)/tau`; arithmetic adjustment sums each deterministic factor times `expm1(marginal_exponent)`, weighted by its accrual fraction. Overflow errors. These are Gaussian moments conditional on the chosen deterministic basis/model assumptions.
6. Model pricing requires the discount curve to exist although the future MTM itself is undiscounted through listed terms. Reported futures price uses the live/settlement mark policy; implied-forward and convexity metrics independently calculate model quantities.

## Risk conventions and selection

When a live quoted mark exists, DV01 clears the quote on a clone if the coarse eligibility predicate sees an explicit convexity adjustment, any supplied vol-surface ID, or a non-Simple compounding mode. It then computes model value and combined or triangular curve risk. The predicate does not verify HW parameters, curve/surface availability or quote conventions; clearing a usable quote can therefore expose missing/invalid model data, and errors propagate. With no live quote or a false predicate the original instrument/value path remains, which can be insensitive to curves. This distinction is observable and must be disclosed with the result.

An IR future option delegates option pricing/Greeks to shared `FutureOptionTerms`. Its rate DV01 is `cash_delta*underlying_price_change_per_bp` plus, for PremiumPaid options, `-T*1bp*NPV` to the relevant cash-payment/expiry discount horizon. Future delivery after exercise has no premium-discount term; futures-style options have none. The price-per-bp conversion is mandatory (typically -0.01 for a 100-minus-rate quote) and supplied by the caller. This is a delta-plus-discount approximation, not full curve repricing.

## Alternatives and tradeoffs

Explicit convexity is supported and can encode an externally calibrated adjustment. The normal-vol term approximation is cheaper and differently parameterized than the overnight Gaussian moment engine. A full stochastic futures calibration is unimplemented. Model-based risk on a quoted position is useful but differs from the derivative of a frozen mark.

## Evidence

- [Future implementation](../../../../finstack-quant/valuations/rates/src/ir_future/types.rs), [future risk selection](../../../../finstack-quant/valuations/rates/src/ir_future/metrics.rs), [option rate DV01](../../../../finstack-quant/valuations/rates/src/ir_future_option/types.rs), [option metric registration](../../../../finstack-quant/valuations/rates/src/ir_future_option/metrics.rs).
- [Convexity tests](../../../../finstack-quant/valuations/rates/tests/ir_future/test_convexity.rs), [reference compounding](../../../../finstack-quant/valuations/rates/tests/ir_future/test_reference_compounding.rs), [metrics](../../../../finstack-quant/valuations/rates/tests/ir_future/test_metrics.rs): formula, information-state and risk behavior evidence.
- [Hull](../../../REFERENCES.md#hull-options-futures), [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [ARRC guide](../../../REFERENCES.md#arrc-sofr-users-guide). Contract registry/defaults and `ListedFutureTerms` remain separate material dependencies.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
