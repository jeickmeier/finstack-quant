# Black equity pricing, discrete dividends and exercise-tree Greeks

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-07`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The native `Black76` registration actually uses spot Black–Scholes for European equities and a Leisen–Reimer binomial tree for American/Bermudan exercise. The European adapter passes escrow-adjusted spot to the closed form. For early exercise it passes raw spot and timed dividends to the [shared escrow-reserve tree](../models-numerical/dividends-and-lattice-exercise.md), which evolves the escrowed component and restores remaining dividends for exercise comparisons. This preserves recombination; it does not establish the approximation's accuracy.

## Scope and input domain

Live single-asset vanilla options with positive strike/spot and finite signed quantity; deterministic discount/dividend inputs and one resolved Black volatility at expiry/strike. The cash-dividend-adjusted spot must be positive.

## Algorithm

1. Set `T=day_count(as_of,expiry)`, `r=−log(D)/T`. Resolve volatility from the active quote override or Black surface at `(T,K)`.
2. If future cash dividends exist through expiry, set `S*=S−Σ Di D(ti)` and `q=0`; otherwise use live spot and optional continuous yield. European call is `S*exp(−qT)N(d1)−Kexp(−rT)N(d2)`, with the corresponding put and `d1=[log(S*/K)+(r−q+σ²/2)T]/(σ√T)`, `d2=d1−σ√T`; shared kernels handle degenerate limits.
3. American/Bermudan pricing instead passes raw spot plus timed dividend amounts to the shared tree. The adapter sets continuous yield to zero when future cash dividends remain, although the generic tree can accept both cash dividends and continuous yield. Bermudan dates are converted using the instrument day count to positive times within maturity; no remaining dates errors. Conversion errors are filtered by the time-collection path.
4. European direct Greeks use the shared analytical kernel. For escrowed dividends, rho adds `delta×Σ ti Di D(ti)` with quote-unit scaling; theta subtracts dividend zero-yield carry times delta. Rho returned here is currency per 1 bp; vega is per one volatility percentage point.
5. Tree delta uses ±1% spot; gamma uses `S×clamp(0.1σ√T,0.02,0.05)`; vega uses ±0.01 vol with the down side floored at `1e-8` and actual-width normalization. Rho uses ±0.01 rate then converts to 1 bp. Daily theta rolls maturity by `1/days_per_year`, removes elapsed dividends/exercise times and reduces spot by dividends crossed; it returns zero inside that final interval.

## Selection, controls and conventions

The tree defaults to 201 requested steps; `model_config.tree_steps` overrides it. [LR construction](../models-numerical/equity-binomial-construction.md) rounds positive even requests upward; the [exercise mapper](../models-numerical/dividends-and-lattice-exercise.md) tests American exercise at every step and maps Bermudan times to nearest steps, rather than inserting exact dates. These are numerical approximations, not accuracy guarantees. A finite-result check follows unit pricing. Native analytical versus selected-model finite-difference risk are separate paths; see [record 08](08-selected-risk-and-implied-vol.md).

Model time defaults to ACT/365F. Date-based discount factors retain the curve's convention and are converted to an effective constant rate satisfying `exp(-r*T)=D`. Rates, continuous yield and annual volatility are decimals; spot/strike are currency per underlying unit and signed quantity scales unit premiums into instrument currency. The scalar implied-volatility override precedes the expiry/strike surface lookup; valuation does not calibrate the model. [Shared input resolution](../valuations-framework/model-resolution.md) distinguishes omitted dividend-yield ID (zero yield) from an explicit missing or monetary scalar (error on the continuous-yield path).

## Limitations, failures and alternatives

Escrowed dividends are a Black approximation and are rejected by stochastic-volatility adapters. Constant expiry-equivalent rates/volatility collapse term structures. Tree Greeks use sizable hard-coded stencils, not the generic metric bump configuration. Uniform maturity parameters and date filtering can affect Bermudan accuracy. PDE and explicit stochastic-volatility engines are implemented alternatives; adoption and convergence limits remain open.

## Evidence

[Black–Scholes](../../../REFERENCES.md#black-scholes-1973), [Merton](../../../REFERENCES.md#merton-1973), [Haug](../../../REFERENCES.md#haug-2007-option-formulas), [American numerical comparison context](../../../REFERENCES.md#broadie-detemple-1996); [test_greeks.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_greeks.rs), [test_put_call_parity.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_put_call_parity.rs) and [test_finance_regressions.rs](../../../../finstack-quant/valuations/tests/instruments/equity_option/test_finance_regressions.rs) locate formula identities and dividend/exercise regressions. These share production numerical components in places; no complete independent LR convergence envelope was reviewed. The [QuantLib-labelled equity fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/equity_option/spx_atm_call_1y_quantlib.json) carries provenance and tolerances whose generation was not requalified in this audit.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [black.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/black.rs)
- [inputs.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs)
- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
