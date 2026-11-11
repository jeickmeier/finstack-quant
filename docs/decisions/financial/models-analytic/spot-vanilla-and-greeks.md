# MODELS-ANALYTIC-01: Spot Black–Scholes price, Greeks and calculation dispatch

**Decision status:** Accepted — complete checked spot BSM pricing, native Greeks and composite-calculation package, 2026-11-09.

**Implementation status:** Observed; reconciliation pending in the source scope and actual product/host/UI consumers below.

**Validation status:** Production evidence inspected/located, not executed. Independent bounded high-precision/scalar references and read-only financial, numerical, source and consumer reviews are reported below; they do not certify production accuracy, product Greeks or a release.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Observed scope and input domain

European calls and puts with constant continuous carry. The observed checked price requires positive finite spot/strike, finite rates and nonnegative expiry/volatility; its Greek API rejects zero expiry/volatility. The accepted contract below instead admits valid zero-boundary Greek queries and returns None for the unavailable complete bundle.

## Accepted decision: checked spot prices, native derivatives and explicit composite results

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[explicit financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md),
[annual volatility-source conventions](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks),
[checked Gaussian/log-tail primitives](../core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains)
and [model versus monetary precision](../conventions/0020-numerical-representation-and-rounding.md).
This record owns the spot BSM formulas and shared calculation behavior, not
forward-model02 or implied-volatility03 algorithms, product lifecycle or a
generic new pricing framework. No runtime changes are authorized here.

1. **Select the constant-coefficient European BSM model explicitly.** Price a
   long call or put per underlying unit under lognormal spot, constant supplied
   volatility and continuous deterministic r/q. Payoff and payment use the same
   modeled expiry. The FX Garman–Kohlhagen interpretation uses domestic r and
   foreign q. No automatic negative-price/normal/shifted model switch, early
   exercise, discrete dividend, stochastic rate/carry or payment-gap model is
   inferred. Actual product selection retains its owner.

2. **Require coherent scalar units and the owner's model clock.** S and K use
   the same price units; sigma is relative annual decimal volatility, r/q are
   continuously compounded annual decimal parameters, and T is an explicitly
   supplied nonnegative model-year coordinate. Require rT/qT and sigma²T to
   represent the intended discount/carry and variance on that coordinate;
   dated adapters own clock mapping or effective-flat input derivation. Remove
   universal ACT/365-style and underlying-day-count inference. Output is a
   binary64 model premium, not Money, a contract total or an FX-converted amount.

3. **Validate all inputs before any identity branch.** Require finite S>0,
   K>0, signed finite r/q and finite sigma/T>=0. Negative r/q, including below−1,
   are valid; negative T/sigma, NaN and infinity error. Validate even unused
   rates/volatility at expiry. Reconcile OptionMarketParams::validate with
   sigma0/T0 admission; stronger model-specific requirements belong to the
   requested operation. Preserve errors rather than returning intrinsic for
   invalid negative inputs or manufacturing default coefficients.

4. **Use exact expiry only for T=0.** Return call max(S−K,0) or put max(K−S,0),
   independent of valid r/q/sigma. Equality gives price0. This is mathematical
   payoff, not proof of exercise, ownership or settlement. Keep the existing
   checked vanilla_expiry_payoff helper's separate S>=0, K>0 domain, including
   an absorbing zero spot; full spot BSM continues to require S>0. An infallible
   OptionType::intrinsic call alone is not validated ingress.

5. **Define zero-volatility price without a proxy variance.** For sigma=0 and
   T>0 let A=S*exp(−qT), B=K*exp(−rT). Return max(A−B,0) for a call or its put
   counterpart, using scaled/logarithmic differences and expm1 where needed.
   Do not round both near-equal legs first, floor sigma or treat a merely small
   positive sigma/T as zero. A mathematically positive result lost to zero is
   a numerical/range error; a genuinely out-of-the-money deterministic payoff
   is valid0. Do not require an unrequested standalone exponential factor to
   fit when its complete discounted amount is representable.

6. **Retain one positive-domain analytic formula.** For T>0 and sigma>0 define
   w=sigma*sqrt(T), m=ln(A/B), d1=m/w+w/2 and d2=m/w−w/2. With Phi the standard
   normal CDF, C=A*Phi(d1)−B*Phi(d2),
   P=B*Phi(−d2)−A*Phi(−d1). These are the mathematical price definitions; their
   literal subtractive evaluation is not the numerical contract. No smile
   dynamics or parameter recalibration is part of these partial derivatives.

7. **Compute geometry in stable coordinates.** Use stable log-price differences
   and scaled rate/time arithmetic for m, preserving near-equal inputs; avoid
   ln(S/K), overflowing sigma² before multiplying T, or d2=d1−w cancellation.
   Compute w and both d values without requiring unrequested intermediates or
   standalone discount factors to fit. Distinguish legitimate tail limits from
   arithmetic loss. If required geometry or cancellation cannot be resolved
   reliably with the selected checked primitives, return a computation/range
   error instead of a reference value, guessed infinity or zero-vol branch.

8. **Remove the negative-price floor and protect small prices.** Evaluate the
   out-of-the-money side through the corresponding Gaussian tails/log tails and
   stable positive differences, then use stable intrinsic/parity for the other
   side. Use the same-function ATM identity A*erf(w/sqrt(8)) when A=B is genuinely
   established, retaining scaled small-argument information or refusing its
   loss. Reuse core Gaussian/erf/log primitives and small private financial
   helpers; add no alternate approximation/model or arbitrary-precision fallback.
   Negative/nonfinite output, unresolved cancellation or lost known-positive
   output errors; do not max(price,0), clip to a bound or silently return zero.
   Interior BSM call/put prices are positive; exact boundary zeros follow4–5.

9. **Make the complete Greek bundle explicitly available or unavailable.**
   Keep bs_price as Result<f64>, independent of any Greek request. Select
   bs_greeks as Result<Option<BsGreeks>>, with None only for otherwise valid
   T=0 or sigma=0 after whole-input validation. This deliberately excludes the
   full singular bundle even where individual partials exist. For T>0/sigma>0,
   compute the complete bundle or error; never turn numerical failure into None.
   Remove strict exercise-indicator/zero/half-delta singular substitutes and
   six individually optional fields. No epsilon-based availability threshold.

10. **Define the six native derivatives literally.** Hold the other supplied
    scalar parameters fixed. With phi the standard-normal density, use:

    | Field | Call | Put |
    | --- | --- | --- |
    | delta = V_S | exp(−qT)*Phi(d1) | −exp(−qT)*Phi(−d1) |
    | gamma = V_SS | exp(−qT)*phi(d1)/(S*w) | Same |
    | vega = V_sigma | A*phi(d1)*sqrt(T) | Same |
    | theta = −V_T | −A*phi(d1)*sigma/(2*sqrt(T))+q*A*Phi(d1)−r*B*Phi(d2) | −A*phi(d1)*sigma/(2*sqrt(T))−q*A*Phi(−d1)+r*B*Phi(−d2) |
    | rho_r = V_r, holding q fixed | T*B*Phi(d2) | −T*B*Phi(−d2) |
    | rho_q = V_q, holding r fixed | −T*A*Phi(d1) | T*A*Phi(−d1) |

11. **Use native derivative units without hidden reporting scales.** Remove
    /100 from vega/rhos and all theta_days_per_year arguments/defaults from this
    kernel, request and hosts. Vega/rhos are per1.0 change in their decimal
    parameter; a per-percentage-point coefficient is0.01 times the raw partial,
    a per-bp coefficient0.0001 times it. Theta is per one supplied model year.
    These are local derivatives, not exact finite-bump P&L. Actual Rust reporting
    owners perform explicit conversion once; do not silently change published
    product metric units or move financial conversion logic into bindings/UI.

12. **Separate model theta from a dated roll.** Theta means −partial V/partial T
    at fixed numeric S,K,r,q,sigma. It is not actual one-day carry/P&L, curve
    aging, a business-day step, observation/fixing arrival or calendar derivative
    under a nonadditive clock. A reporting divisor changes units only; it does
    not choose a day-count convention or rescale input volatility. Remove the
    universal calendar-day default and market-by-product denominator claims.

13. **Keep delta and FX interpretation honest.** Spot delta is premium-unadjusted
    V_S. With q<0, its magnitude can exceed1; the model carry bound is exp(−qT),
    not1 and not merely a statement about q relative to r. In FX, S is domestic
    currency per foreign unit and premium is domestic per foreign underlying
    unit. Product owners perform forward-delta/premium-adjustment, quote
    orientation, domestic/foreign contract scaling and smile reselection. No
    such conversion is inferred from the BSM bundle or a currency identifier.

14. **Compute Greeks in their final scale, with truthful failures.** Use log
    density/tails and scaled complete products for delta/gamma/vega/rhos and
    stable signed assembly for theta; do not underflow a normal tail/density
    before multiplying a large carry/spot factor or overflow an unused denominator.
    Require finite outputs and the proper interior signs, including positive
    gamma/vega; loss of a known-nonzero sensitivity to zero errors. Theta may
    round to zero through signed cancellation without proving exact zero. Reuse
    checked shared primitives; no gamma/vega floor, NaN sentinel, missing
    substitution or uniform relative-accuracy claim near cancellation.

15. **Expose checked kernels only.** Remove public bs_price_unchecked,
    bs_greeks_unchecked and any public unchecked BSM geometry bypass. Private
    helpers may share admitted state, but all actual pricing/Greek consumers
    propagate canonical input/computation errors. Retain the needed existing
    d1_d2 helper only as checked fallible regular-domain geometry, shared with
    its actual tree/FX callers rather than copied formulas. Remove duplicate formulas and
    error-to-default/zero recovery; do not keep compatibility wrappers. Input
    admission is necessary, not a promise that every finite input has a
    numerically representable or successfully resolvable requested output.

16. **Guard result and input-state boundaries.** Keep computed BsGreeks immutable
    with read-only get_* inspection, no fabricated all-zero Default or unchecked
    mutable construction. Validate finite restored fields, strictly positive
    gamma/vega, nonzero delta/rhos and coherent call/put derivative signs;
    theta0 is valid. Require strict field shapes and no unknown/duplicate wire
    keys before collapse, matching the regular computed bundle's invariants.
    Validated result shape does not prove it was calculated from a particular
    option or certify derivative accuracy. Reconcile actual parameter ingress/
    serde/host validation with3; do not impose a lognormal domain on unrelated
    forward/normal model inputs through a shared helper.

17. **Retain the existing calculator as an explicit composite operation.**
    option_calculation requests price-or-IV PLUS its model's Greek bundle,
    rather than being described as price-only. Rename result implied_vol to vol:
    it echoes supplied volatility for a price solve and contains the solved
    volatility for an inverse. Make its whole greeks field nullable under the
    canonical model availability rule. BSM valid singular price may succeed
    with greeks=None; positive-domain Greek computation failures still fail
    this composite. Users needing price only call bs_price. No hidden catch,
    extra output flags or partial-result/status framework is selected.

18. **Keep forward and inverse laws with their next records.** The shared
    calculator shape does not adopt02's forward price/discount/Greek domains or
   03's IV bounds, clocks, root selection and residual gates. Each branch calls
    its canonical checked owner and preserves errors. A price solve retains its
    admitted supplied vol; an IV solve's price field is the supplied target,
    not proof of repricing accuracy. Unsupported singular inversion cannot be
    rescued by this record's valid boundary price. Reconcile the shared result
    and future owner availability without retaining contradictory aliases.

19. **Leave real product economics and risk selection with owners.** BSM scalar
    partials do not implement curve-chain-rule DV01, volatility-surface dynamics,
    discrete-dividend corrections, funding/CSA selection, exercise/settlement
    lifecycle, notional/position sign or cash/Money rounding. Actual equity/FX
    adapters qualify those inputs and transformations explicitly. Do not infer
    actual settlement eligibility from the T0 payoff or make a model reference
    a default product-selection decision under ADR0027.

20. **Reconcile actual product and replication callers.** Update equity option
    price/direct-Greek and discrete-dividend rho/theta chain rules, FX option
    price/delta/risk and contract scaling, barrier/parity and variance-replication
    callbacks and geometric Asian pricing using the unchecked kernels. Reconcile
    d1_d2 in the Leisen–Reimer tree and FX digital/vanilla callers, keeping their
    boundary choices at their owners. Preserve each owner-selected public
    report unit by explicit Rust conversion, rather than applying old /100/day
    assumptions to newly raw derivatives. Propagate absent singular bundles and
    computation failures without a made-up product Greek or zero-price fallback.
    Optional instrument/exercise/smile models retain separate financial review.

21. **Reconcile existing hosts, UI and strict state together.** Update Rust
    re-exports/results, Python/WASM wrappers and errors, stubs/declarations,
    generated schemas, parity ledger and live UI calculator result consumers.
    Preserve canonical names, required inputs, nullable whole bundles and vol
    terminology; remove theta defaults and stale per-day/per1% labels. Bindings
    and UI do conversion/presentation, not independent financial formulas or
    implicit unit scaling. Add no host-only model or compatibility surface.

22. **Qualify the selected profile independently.** Check model price bounds
    and put–call parity, native derivative identities/PDE and unit conversions,
    signed rates, exact boundary admission/availability, thin ATM premiums,
    extreme ratios/total variance and scaled tails/Greeks, strict state/hosts
    and composite failures. Use high-precision and primary-source comparisons
    with stated model coordinates and scale-aware errors. No global ULP or
    settlement-grade guarantee follows from finite results, a price floor,
    matched self-helper tests or agreement with one external implementation.
    Runtime, broad numerical-domain and complete product qualification remain
    pending; this acceptance changes documentation only.

## Independent bounded reference checks

Independent Decimal/high-precision series and binary64 expressions on2026-11-09,
without importing production code, checked:

- At S=K=100,r=q=0,T1,sigma1e−18, the ATM identity gives positive price
  approximately3.989422804014327e−17, while direct rounded CDF subtraction can
  return0. At sigma0,S=K1,T1,q0,r=2^-54, deterministic price is approximately
  5.551115123125783e−17, despite separately rounded discounted legs both being1.
- For sigma1e200/T1e−300, total standard deviation1e50 is finite although
  sigma² overflows. At S=K1/r=q0 the price tends to1; the old geometry can
  incorrectly produce0. S1e−300/K1e300/sigma100/T1/r=q0 has a positive call near
  1e−300 despite an underflowed S/K ratio. Scaled discounted amounts can also
  remain representable when their standalone exponential factor underflows.
- At S=K1,T1,sigma80,r=q−700, raw gamma is approximately1.855119491425422e−46,
  vega1.484095593140338e−44 and rho_r3.707924417894682e−46. The density/tail at40
  rounds to0 before large carry scaling; these requested sensitivities do not.
- At expiry the payoff kink has no ordinary spot delta/gamma, and ATM
  small-time gamma/theta can diverge. Away from the kink, ITM call theta tends
  qS−rK and need not be0. Thus unavailable complete bundles do not assert that
  every individual derivative is undefined or zero.
- Ordinary nonzero-carry call/put reference values, all six raw derivatives,
  parity/PDE identities and per-pp/bp conversions were evaluated independently.
  These are bounded definitions/coordinate checks, not production test runs,
  domain-wide accuracy bounds or approval of product risk conventions.

## Observed choice and rationale

The kernel uses the Black–Scholes–Merton formula. Analytic derivatives avoid bump noise; this is an inferred benefit, not evidence of adoption. The public calculation request explicitly chooses BlackScholes, Black76 or Bachelier and either price or implied volatility.

## Observed algorithm

For T>0 and σ>0, d1=[ln(S/K)+(r−q+σ²/2)T]/(σ√T), d2=d1−σ√T; C=S exp(−qT)N(d1)−K exp(−rT)N(d2), with the analogous put. Expiry uses spot intrinsic; zero volatility uses intrinsic of discounted spot and strike. Finite prices are floored at zero. Delta is ±exp(−qT)N(±d1), gamma=exp(−qT)φ(d1)/(Sσ√T), vega=S exp(−qT)φ(d1)√T/100; theta combines diffusion and carry terms, then divides by configured days/year; rate/dividend rho divide analytic derivatives by 100. `option_calculation` first solves price or volatility, then computes Greeks at that volatility; it cannot return price alone when Greeks reject the domain.

## Observed selection and controls

`bs_price` and `bs_greeks` validate; `_unchecked` variants expose deterministic/expiry behavior without those checks. Theta defaults to core calendar days/year through calculation dispatch. Instrument selection is separately described in [ADR 0027](../valuations-equity-and-composition/0027-vanilla-option-model-selection.md). No exercise or discrete-dividend model is selected here.

## Observed conventions

Rates and σ are annual decimal inputs, time is a supplied year fraction, premium is currency per underlying unit. Spot delta/gamma are per spot unit; vega/rhos are per percentage-point change, theta per configured day. In contrast, the forward calculation returns undiscounted unit-vol Greeks even when Black76 price uses df.

## Observed limitations, failure and fallback

At expiry unchecked spot delta is zero exactly at the strike (strict >/<); raw Black/Bachelier boundary delta uses a different tie convention. `bs_price` permits zero vol/time but `option_calculation` fails when its mandatory checked spot Greeks cannot be computed. No stochastic carry, smiles, early exercise or cash-dividend scheduling occurs in this kernel.

## Alternatives and tradeoffs

Black76 and Bachelier are implemented request alternatives. Bump Greeks or smile-adjusted Greeks are possible but not selected here; analytic sensitivities hold the supplied volatility fixed.

The accepted package keeps checked BSM prices and native derivatives separate,
with explicit singular availability and a deliberately composite calculator.
Raw derivative units replace embedded report scales; stable checked evaluation
replaces unchecked/floored formulas. Forward/inverse and real-product choices
retain their owners rather than being adopted through shared dispatch.

## Implementation and validation evidence

[vanilla.rs](../../../../finstack-quant/models/src/closed_form/vanilla.rs), [black.rs](../../../../finstack-quant/models/src/closed_form/black.rs) and [calculation.rs](../../../../finstack-quant/models/src/closed_form/calculation.rs) own formulas, singular branches, units and dispatch. Inline `test_bs_greeks_call`, `test_bs_price_put_is_non_negative_for_deep_otm_case`, and `price_solve_matches_the_kernels` are formula/boundary/kernel-routing evidence. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) and [gen_vol_golden.py](../../../../finstack-quant/models/tests/golden/data/gen_vol_golden.py) provide separate QuantLib/mpmath price fixtures; they do not independently certify all Greeks. References: [Black–Scholes](../../../REFERENCES.md#black-scholes-1973), [Merton](../../../REFERENCES.md#merton-1973).

- [Official Strata formula/Greek source](../../../REFERENCES.md#bsm-formula-derivative-coordinates) was inspected for regular interior formulas and derivative coordinates. Its b=r−q; its domestic rho varies r and b together holding q fixed, whereas our rho_q is minus carryRho. Its thresholds/reference-value/tail shortcuts are not adopted. Historical BSM/Merton/Garman papers remain bibliographic pointers; exact historical pages/equations were not independently verified here.
- Actual parameter and product evidence: [OptionMarketParams](../../../../finstack-quant/models/src/types.rs), [equity direct pricing/Greeks](../../../../finstack-quant/valuations/equity/src/equity_option/pricing/black.rs) and [FX pricing/Greeks](../../../../finstack-quant/valuations/fx/src/fx_option/pricer.rs). Read-only independent numerical, financial, source and host/consumer reviews challenged the whole proposal; no production calculation or product benchmark was executed.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
