# VALUATIONS-EQUITY-AND-COMPOSITION-07: Black equity pricing, discrete dividends and exercise-tree Greeks

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-07`.
- **Decision status:** Accepted 2026-11-14 — complete live equity option pricing, dividend, exercise-tree and direct risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary references inspected. Independent dated European price and Greek arithmetic, finite LR rollback and exercise-phase checks completed. Market, runtime and host qualification pending.

## Accepted decision for checked live equity option pricing and Greeks

1. **Select the actual live model under its own key.** Use `black_scholes`
   for the European baseline and the existing `tree` key for American and
   Bermudan exercise, with an explicit Leisen–Reimer or CRR construction.
   The product default selects the route from authored exercise style.
   Missing controls, incompatible terms or failed pricing propagate. Remove
   the equity `black76` alias. Apply record 06's checked lifecycle first,
   so resolved cash or delivery uses its own valuation and active inputs.
   [Accepted dispatch](../valuations-framework/pricing-lifecycle.md),
   [accepted equity option lifecycle](06-option-lifecycle.md),
   [vanilla selection overview](0027-vanilla-option-model-selection.md).

2. **Separate European fixing and payment.** Let `A` be the qualified
   prepaid equity on the selected dividend basis, `E` the fixing cut and
   `P` the fixed cash payment date. Form fixing forward `F=A/D(V,E)` and
   apply the checked Black unit expectation on `F`, strike `K` and total
   standard deviation `w` through fixing. With deterministic dated factors,
   the signed cash premium uses `D(V,P)` on that expected payoff. Qualify
   the actual physical exchange before using a model whose exercise payoff
   is intrinsic. A live payoff with unsupported settlement economics is
   unavailable. Check positive underlying, prepaid amount, strike and
   live model clock, retain native units, and apply side and size once.
   [Black and Scholes](https://doi.org/10.1086/260062),
   [checked unit forward expectation](../models-analytic/forward-normal-shifted.md),
   [accepted equity carry](02-spot-and-carry.md).

3. **Elect the European dividend and variance assumptions explicitly.**
   Select no distributions, a qualified continuous yield or deterministic
   expiry-window cash escrow. In the cash approximation subtract the PV
   of rights detached by the fixing phase, discounting each dividend on
   its actual payment date even when it pays after fixing. Retain event
   identity, entitlement and source basis. Select one admitted native
   scalar volatility or exact compatible Black strike-surface quote.
   Qualify volatility of the escrow-adjusted underlying. Bridge a source
   to the model clock by preserving total variance. Zero volatility uses
   the exact deterministic price and leaves the complete analytic Greek
   bundle unavailable. [Dated cash events](../core-market/dividend-events.md),
   [explicit volatility sources](../valuations-framework/model-resolution.md),
   [checked analytic price and derivative domains](../models-analytic/spot-vanilla-and-greeks.md).

4. **Require exact event placement and an explicit tree construction.**
   Use a declared homogeneous carry law, literal interval count and checked
   node-evaluation budget. Leisen–Reimer requires odd count. CRR accepts
   either parity under its stochastic support condition. Validate exact
   cum/ex and exercise phase placement on the declared clock. Remove date
   filtering, nearest nodes, automatic parity adjustment and implicit tree
   replacement. A halfway event cannot align on an odd Leisen–Reimer tree.
   Use the record 002 escrow reserve through its declared underlying horizon,
   including eligible events after option expiry. Discount each cash claim
   from its actual pay date to the ex boundary on its qualified basis. Use
   the positive risky component for diffusion and reconstruct full spot at
   exercise. Center stochastic Leisen–Reimer on `K−R_terminal`, which must
   be positive. Retain eligible phases for deterministic volatility zero.
   [Checked lattice construction](../models-numerical/equity-binomial-construction.md),
   [escrow and exercise ordering](../models-numerical/dividends-and-lattice-exercise.md).

5. **Compute direct Greeks on the priced model.** In the European
   branch, differentiate through prepaid equity with selected strike
   volatility held fixed. With unit Black function `B` and fixing
   sensitivity `F_S`, delta and gamma are `D(V,P) B_F F_S` and
   `D(V,P) B_FF F_S²` on the admitted affine prepaid basis. Scale once
   to signed product value. Black model vega reports cash per `+0.01`
   native model volatility. An active surface quote move has a separate
   source basis and clock. Tree risk uses checked matched reprices on
   explicit steps and actual represented widths with the same selected
   tree and prepared event geometry. Remove hard-coded bumps and floors.
   A declared local rho is per positive bp on its own rate direction.
   Keep curve DV01 on actual dated curve sources. Keep local model time
   sensitivity distinct from the dated cash-inclusive horizon calculation.
   Alternative-model risk, higher Greeks and implied volatility belong
   to record 08. [Accepted option sensitivities](../valuations-framework/option-sensitivities.md),
   [selected-model risk and implied volatility](08-selected-risk-and-implied-vol.md).

6. **Qualify price and risk on matched assumptions.** Compare explicitly
   chosen aligned grids on declared price and sensitivity tolerances.
   Match reserve, transformed strike and volatility basis when comparing
   a tree with an analytic price. The European expiry-window escrow and
   the tree's underlying reserve can give different premiums at the same
   parity. Retain selected model, calendar dates, source identity, units,
   represented steps and grid evidence through native, registered and
   existing Python and WASM paths. [Leisen and Reimer](https://doi.org/10.1080/13504869600000015).

## Observed behavior

### Choice and rationale

The native `Black76` registration actually uses spot Black–Scholes for European equities and a Leisen–Reimer binomial tree for American/Bermudan exercise. The European adapter passes escrow-adjusted spot to the closed form. For early exercise it passes raw spot and timed dividends to the [shared escrow-reserve tree](../models-numerical/dividends-and-lattice-exercise.md), which evolves the escrowed component and restores remaining dividends for exercise comparisons. This preserves recombination; it does not establish the approximation's accuracy.

### Scope and input domain

Live single-asset vanilla options with positive strike/spot and finite signed quantity; deterministic discount/dividend inputs and one resolved Black volatility at expiry/strike. The cash-dividend-adjusted spot must be positive.

### Algorithm

1. Set `T=day_count(as_of,expiry)`, `r=−log(D)/T`. Resolve volatility from the active quote override or Black surface at `(T,K)`.
2. If future cash dividends exist through expiry, set `S*=S−Σ Di D(ti)` and `q=0`; otherwise use live spot and optional continuous yield. European call is `S*exp(−qT)N(d1)−Kexp(−rT)N(d2)`, with the corresponding put and `d1=[log(S*/K)+(r−q+σ²/2)T]/(σ√T)`, `d2=d1−σ√T`; shared kernels handle degenerate limits.
3. American/Bermudan pricing instead passes raw spot plus timed dividend amounts to the shared tree. The adapter sets continuous yield to zero when future cash dividends remain, although the generic tree can accept both cash dividends and continuous yield. Bermudan dates are converted using the instrument day count to positive times within maturity; no remaining dates errors. Conversion errors are filtered by the time-collection path.
4. European direct Greeks use the shared analytical kernel. For escrowed dividends, rho adds `delta×Σ ti Di D(ti)` with quote-unit scaling; theta subtracts dividend zero-yield carry times delta. Rho returned here is currency per 1 bp; vega is per one volatility percentage point.
5. Tree delta uses ±1% spot; gamma uses `S×clamp(0.1σ√T,0.02,0.05)`; vega uses ±0.01 vol with the down side floored at `1e-8` and actual-width normalization. Rho uses ±0.01 rate then converts to 1 bp. Daily theta rolls maturity by `1/days_per_year`, removes elapsed dividends/exercise times and reduces spot by dividends crossed; it returns zero inside that final interval.

### Selection, controls and conventions

The tree defaults to 201 requested steps; `model_config.tree_steps` overrides it. [LR construction](../models-numerical/equity-binomial-construction.md) rounds positive even requests upward; the [exercise mapper](../models-numerical/dividends-and-lattice-exercise.md) tests American exercise at every step and maps Bermudan times to nearest steps, rather than inserting exact dates. These are numerical approximations, not accuracy guarantees. A finite-result check follows unit pricing. Native analytical versus selected-model finite-difference risk are separate paths; see [record 08](08-selected-risk-and-implied-vol.md).

Model time defaults to ACT/365F. Date-based discount factors retain the curve's convention and are converted to an effective constant rate satisfying `exp(-r*T)=D`. Rates, continuous yield and annual volatility are decimals; spot/strike are currency per underlying unit and signed quantity scales unit premiums into instrument currency. The scalar implied-volatility override precedes the expiry/strike surface lookup; valuation does not calibrate the model. [Shared input resolution](../valuations-framework/model-resolution.md) distinguishes omitted dividend-yield ID (zero yield) from an explicit missing or monetary scalar (error on the continuous-yield path).

### Limitations, failures and alternatives

Escrowed dividends are a Black approximation and are rejected by stochastic-volatility adapters. Constant expiry-equivalent rates/volatility collapse term structures. Tree Greeks use sizable hard-coded stencils, not the generic metric bump configuration. Uniform maturity parameters and date filtering can affect Bermudan accuracy. PDE and explicit stochastic-volatility engines are implemented alternatives; adoption and convergence limits remain open.

### Evidence

[Black–Scholes](../../../REFERENCES.md#black-scholes-1973), [Merton](../../../REFERENCES.md#merton-1973), [Haug](../../../REFERENCES.md#haug-2007-option-formulas), [American numerical comparison context](../../../REFERENCES.md#broadie-detemple-1996); [test_greeks.rs](../../../../finstack-quant/valuations/equity/tests/equity_option/test_greeks.rs), [test_put_call_parity.rs](../../../../finstack-quant/valuations/equity/tests/equity_option/test_put_call_parity.rs) and [test_finance_regressions.rs](../../../../finstack-quant/valuations/equity/tests/equity_option/test_finance_regressions.rs) locate formula identities and dividend/exercise regressions. These share production numerical components in places; no complete independent LR convergence envelope was reviewed. The [QuantLib-labelled equity fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/equity_option/spx_atm_call_1y_quantlib.json) carries provenance and tolerances whose generation was not requalified in this audit.

### Relationship to the accepted cash-dividend source boundary

[CORE-MARKET-13's complete accepted package](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) requires identified
cash-per-share events with explicit ex/pay dates. Reconcile high-level ex-only
numeric dividend inputs through a financially qualified source adapter, retaining
underlying/share/currency and announcement/forecast meaning. A delayed payment
is not automatically an instantaneous cash jump; map claim value and cum/ex
exercise phases only under the selected model or reject unsupported cases.
CORE-MARKET-13 was accepted on **2026-11-08**. The accepted product package
above supplies its own model and risk mapping to the dividend source.

## Independent arithmetic and qualification

- With spot 100 and two eligible dividends of 2 and 3 with payment factors
  `.975` and `.965`, prepaid equity is `95.155`. At fixing factor `.98`,
  payment factor `.96`, strike 98, model time `.25` and volatility `.20`,
  call is `3.3169083101` and put `4.1838470856`. The held-vol call delta,
  gamma and vega per vol point are `.47316209885`, `.04103268892` and
  `.18576470800`. Discounting with the fixing factor gives call `3.3860105665`.
- In a zero-carry LR case with spot and strike 100, one-year model time,
  risky volatility `.20`, cash 8 crossing at one third of expiry and cash
  10 after expiry inside the reserve, the risky initial spot is 82 and
  effective terminal strike 90. At three intervals the ex-only Bermudan
  call is `3.5591634802` and the cum-only call `4.8563614441`. The aligned
  nine-interval cum value is `4.6405975250`.

Qualify actual market inputs and live payoff terms, escrow basis, exact
events, independent exercise pricing and grid sensitivity, and complete
runtime and host routes. Repository runtime and host suites remain unrun.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [black.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/black.rs)
- [inputs.rs](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/inputs.rs)
- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
