# VALUATIONS-RATES-10-RATE-FUTURES: Rate futures projection, convexity and option rate risk

- ID: VALUATIONS-RATES-10-RATE-FUTURES
- Decision status: **Accepted — complete checked rate futures and futures option risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, numerical, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked rate futures and futures option risk

1. **Resolve the exact listed rate contract.** Scope the underlying to a
   cash-settled future with annual-decimal benchmark rate and
   `100−100R` price points. Bind venue and exact contract identity,
   index, currency, position, quantity, point multiplier, applicable
   ticks, calendars and authoritative dated terms. Require either
   complete authored terms or a qualified venue edition, version and
   effective date. Keep the actual reference interval `[S,E)`,
   benchmark fixing and publication rules, last trading session,
   official final mark authority, and clearing/payment dates separate.
   Validate each branch's actual chronology; a term reset may precede
   reference start, and an overnight reference may extend past last
   trading. Remove inferred fixing from last trade, generic
   fixing-plus-two-calendar-days and delivery-month fallbacks, and
   default product, currency or calendar selection. A tick or final
   rounding rule applies only at its owned stage.
   [Convention selection](../valuations-framework/convention-selection.md),
   [CME one-month SOFR](https://www.cmegroup.com/rulebook/CME/IV/400/461.pdf),
   [CME three-month SOFR](https://www.cmegroup.com/rulebook/CME/IV/400/460.pdf)
   and [ICE Euribor contract terms](https://www.ice.com/products/38527986/Three-Month-Euribor-Futures/specs).

2. **Use one publication-qualified benchmark projection.** Elect term
   simple, weighted overnight arithmetic, or unshifted overnight
   compounding with zero lookback, only where the venue or authored
   contract supports it. Exclude implicit shift, cutoff or reuse
   rules outside the chosen profile. Bind the exact `IndexId`, value
   date, publication session and admitted fixing version in one
   frozen snapshot. Use required published fixings, including any
   vintage or first-print rule. Project only when the publication
   phase permits it and the exact typed source covers the full
   benchmark interval on its native clock. A past value date can
   still be unpublished; calendar equality proves neither status.
   Missing history or failed required projection errors. Retain
   hypothetical observations only under an explicitly admitted
   scenario. For term simple, take the admitted fixing or the checked
   dated gross factor increment divided by the actual benchmark
   accrual. Remove curve-ID fixing keys and unconditional forward
   lookups when the reference is fully known.
   [Fixing eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md),
   [forward factors](../core-market/forward-projection.md) and
   [New York Fed publication and calendar-day rules](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates).

3. **Preserve full overnight quotes and partial contract weights.**
   Compile actual business-day blocks and intersect them with the
   contract window. Retain the preceding fixing for a closed start
   where the contract admits it, the full benchmark interval and
   accrual `α_i`, and the actual overlapping contract weight `w_i`
   separately. A weekend is one quote block, not one compounding
   factor per calendar day. Use the contract's index basis and
   `τ=Σw_i>0`: arithmetic `Σr_i w_i/τ`, compounded
   `[Π(1+r_i w_i)−1]/τ` with checked positive factors and stable
   growth arithmetic. Preserve signed rates and unrounded model
   points. For a parallel bump of remaining benchmark quotes use
   `Σ_live w_i/τ` for arithmetic and
   `Σ_live w_i Π_(j≠i)(1+r_j w_j)/τ` for compound, with known
   observations held and all occurrences retained. A live term quote
   has sensitivity one; fixed quotes have zero. This local quote
   derivative is separate from zero-rate curve risk and does not
   scale the convexity adjustment.
   [Accepted overnight projection](../valuations-framework/overnight-projection.md),
   [overnight block construction](../cashflows/07-overnight-compounding.md).

4. **Elect the live model's convexity basis.** A model mark with
   unresolved observations requires either an explicit finite signed
   adjustment to the final annualized reference rate for this exact
   contract and information cut (zero may be explicitly elected),
   or a qualified Hull–White expectation. Add a supplied adjustment
   once; do not multiply it by the live quote derivative. Hold it
   fixed in direct curve shocks. A fully fixed rate has zero
   remaining convexity and needs no inactive model inputs. A
   Hull–White request binds one complete authored or qualified fitted
   block, actual origin and ACT/365F model clock, finite nonnegative
   mean reversion and scalar or qualified dated scheduled absolute
   short-rate sigma. Use the actual discount and projection roles
   with explicit deterministic multiplicative basis if distinct.
   Preserve original schedule support and separately evaluate the
   dated curve clocks and contractual weights. This profile uses a
   money-market measure expectation on qualified linear daily
   margining terms; keep the model unrounded and apply official
   rounding only at its venue-owned stage.
   A permitted unpublished quote with fixing time before the model
   origin is outside this stochastic profile; no time clipping.
   Remove the Normal option volatility proxy, its formula
   `0.5 σ² T_start T_end` and the 5% cap.
   [Hull–White state and clock](../models-analytic/rate-clock-and-projection.md).

5. **Use the actual term fixing in the selected Gaussian law.** For
   admitted `v≤f≤S<E`, let `F` be the prepared simple forward,
   `τ` its benchmark accrual, `G_0=1+τF>0` and
   `L=B(f,E)−B(f,S)` evaluated in factored form. With the checked
   conditional variance `V(v,f)` and state/integral covariance
   `J(v,f)`, set `z=L[J(v,f)+B(f,E)V(v,f)]`. The increment added
   to the projected annual rate is `G_0 expm1(z)/τ` on the
   qualified deterministic-basis law. Use the actual early fixing;
   when `f=S` this reduces to the accepted accrual-start law.
   Exact zero horizon or supported zero variance gives a literal zero
   increment after admission. Keep the inverse quoted-futures-to-forward
   direction distinct: given the actual futures-implied rate
   `R_f`, subtract `(1+τ R_f)(−expm1(−z))/τ` with the same
   validated terms. No approximation based only on model time to
   accrual start or on a market option sigma.
   [Accepted Hull–White kernels](../models-analytic/hull-white-analytic-kernels.md),
   [Henrard term futures, theorem 1](https://econwpa.ub.uni-muenchen.de/econ-wp/fin/papers/0503/0503005.pdf#page=3).

6. **Use discrete Gaussian moments only on the supported overnight
   blocks.** Reuse the same prepared observations for projection and
   convexity. For each unresolved modeled fixing at full interval
   start `t_i`, use full end `u_i`,
   `b_i=B(t_i,u_i)`, `G_i=1+α_i F_i`,
   `z_i=b_i[J(v,t_i)+b_i V(v,t_i)]` and the actual OU covariances
   `C_ij`. Arithmetic correction is
   `Σ_i w_i G_i expm1(z_i)/(τ α_i)`; partial weights can enter
   this linear expectation. For compounded terms admit automatic
   moments only when each genuinely stochastic block has
   `w_i=α_i`. Then `z=Σ_i z_i+Σ_(i<j)b_i b_j C_ij` and
   the correction is `G_proj expm1(z)/τ`, where `G_proj` is
   full contract gross growth on the projected and known factors,
   including admitted known partial factors. A clipped stochastic
   factor is affine in gross growth, so this product formula does
   not apply. Such a compound contract can use the explicit
   adjustment profile. Use checked stable sums and moments, bounded
   actual work and no dropped observations, covariance terms or
   nonfinite fallbacks. Qualify the assumed benchmark basis and
   money-market measure for the actual contracts.
   [Hull–White conditional moments](../models-analytic/hull-white-analytic-kernels.md)
   and [Henrard on overnight futures convexity](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=3134346).

7. **Separate model points, official marks and settled cash.** Compute
   unrounded theoretical points `100(1−R_projected−adjustment)` only
   for an explicitly selected model mark or stand-alone diagnostic.
   Keep projection, effective adjustment and model price separately
   labeled. The rate implied by an observed IMM mark is its quoted
   futures rate; it is not the pre-convexity model projection.
   A live observed mark requires no unused projection or
   model inputs. After last trading require the authorized final mark
   as applicable, including its availability and clearing precision.
   A modeled fully fixed benchmark does not replace that mark.
   Use signed live variation `side × contracts × multiplier ×
   (mark−reference)` with the trade fill for a new trade or last
   applied clearing mark for a carried position. Distinguish
   cumulative P&L, known unpaid finalized variation and already
   booked/settled amounts. Live daily marks are undiscounted; only
   an eligible unpaid fixed claim uses actual dated discounting
   and factor one at an included same-day cut. Remove the unused
   discount lookup. Require proof of closure before an empty cash
   schedule or structural zero; uncertain live model amounts are
   unavailable as authorized cash.
   [Accepted listed products](../valuations-framework/listed-products.md),
   [CME clearing arithmetic](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf).

8. **Choose the actual futures risk experiment.** Report live
   point delta as signed cash per futures-price point. A quote-held
   curve experiment has zero live mark sensitivity when that is
   proved; fixed pending cash can still have discount exposure.
   Require an explicit independently admitted model-risk request
   to risk a live observed position on the theoretical basis.
   Hold the observed headline mark and its reference unchanged.
   Use `dv01`, `pv01` and `bucketed_dv01` for the selected
   combined, typed per-source and triangular direct shocks, with
   explicit controls and signed native currency per positive bp.
   Shock only active typed sources, once per shared evaluator,
   and rebuild the admitted model and curve-responsive
   observations without changing actual history, economic
   parameters or the margining basis. Separately identify and
   explicitly authorize a market quote replay with complete center
   and side qualifications. Remove the clear-quote-on-clone
   heuristic, coarse eligibility test and inactive fallback.
   Missing required data or model failures propagate on their
   selected basis. [Shared direct and replay risk](../valuations-framework/finite-difference-rates.md).

9. **Separate mapped futures-option risk from discount risk.** Reuse
   the checked `FutureOptionTerms` pricing, exercise, settlement,
   grid and Greeks on the exact underlying contract, point units
   and lifecycle. Use one signed model point delta per price point
   and consume a finite signed point move per positive bp only for
   a requested `mapped_rate_dv01` on the active price axis.
   Identify the mapped rate and source. The `−.01` IMM mapping is
   a bp of the full contract settlement rate, separate from a
   curve-node shift. Compute `discount_dv01` separately with a
   checked dated bump of the actual discount source, holding
   the underlying mark and other held inputs fixed. Reprice the
   same eligible premium/settlement and pending cash on both
   sides; use the qualified American lattice and its refinement
   checks when required. Live futures-style quote PV has no
   discount channel; eligible unpaid fixed cash may still have it.
   After exercise, delegate delivered futures once with their
   own reference and state; a fixed cash claim needs no live
   delta, volatility or bp mapping. Retire the construction-time
   mapping requirement and the generic sum of mapped delta
   and `−T·1bp·NPV`. Combined risk needs its own explicit
   matched scenario and full repricing. Time and exercise
   boundaries follow the accepted listed and holder horizon rules.
   [Listed option risk and lifecycle](../valuations-framework/listed-products.md),
   [CME option margining styles](https://www.cmegroup.com/education/articles-and-reports/a-primer-on-margining-styles-for-options)
   and [CME SOFR option delivery and point units](https://www.cmegroup.com/rulebook/CME/V/450/460A/460A.pdf).

10. **Reconcile existing product, calibration and host routes.**
    Preserve the contract edition, observation status and weights,
    selected quote or model basis, actual risk axes and controls,
    clocks, authority and cash provenance through Rust, existing
    Python/WASM routes, reports, clearing/cash exports, schemas,
    declarations and parity. Align the existing
    [calibration builder](../calibration-framework/05-futures-materialization.md)
    with resolved terms, mark units, actual reference end and
    explicit convexity election. Keep recipe construction and
    fit qualification with calibration. Independently qualify
    real venue calendars and publication vintages, negative rates,
    full and partial blocks, early term fixing, the zero stochastic
    limit, Gaussian moments and support, official/unpaid lifecycle,
    direct versus mapped risk and the option's actual discount
    horizon. Same-kernel formula tests or quote roundtrips do not
    certify market conformity.

## Observed behavior

### Choice, scope and rationale

Rate futures project an exchange reference rate, add an explicitly supplied or calculated futures/forward convexity adjustment, and map to `100*(1-rate-adjustment)`. Term simple rates, daily arithmetic averages and compounded-in-arrears with zero lookback are supported; lockout/shift/lookback alternatives are rejected. Daily settlement and mark selection are shared [listed-product methodology](../valuations-framework/listed-products.md), not bond-style discounted principal valuation.

### Algorithm and controls

1. Resolve fixing from the explicit field or last trading date; absent reference start is fixing plus two **calendar** days, and absent end adds `delivery_months`. Validate ordered dates. Term fixing cannot follow period start; overnight final fixing must lie within the reference period.
2. A term fixing strictly before valuation requires an exact historical observation and has zero forward sensitivity. Otherwise project the period's simple forward and set parallel sensitivity to one. Daily arithmetic/compounded rates delegate to the shared overnight projector, retaining already fixed observations and calculated remaining-forward sensitivity. Explicit `index_id` chooses fixings; otherwise the forward curve ID is reused.
3. Fully fixed projections have zero convexity. An explicit decimal adjustment is multiplied by remaining parallel-forward sensitivity.
4. Without explicit adjustment, a term future requires a Normal, strike-axis volatility surface, sampled at forward-curve time to start and projected rate. Vol must be in `[0,0.05]` decimal absolute rate units. Add `0.5*sigma^2*T_start*T_end`, scaled by remaining sensitivity. This is the implemented approximation, not a Black-vol formula.
5. Automatic overnight convexity requires explicit constant Hull–White mean reversion and sigma, rejecting sigma schedules and substitution of a surface for model parameters. For each unfixed daily observation use ACT/365F times, bond loading `b=B(t,u)`, state variance and covariance. Accumulate marginal exponent `b*0.5*sigma^2*B(0,t)^2 + b^2*Var[x_t]`, plus pairwise `b_i*b_j*Cov[x_i,x_j]`. Compounded adjustment is `(1+projected_rate*tau)*expm1(total_exponent)/tau`; arithmetic adjustment sums each deterministic factor times `expm1(marginal_exponent)`, weighted by its accrual fraction. Overflow errors. These are Gaussian moments conditional on the chosen deterministic basis/model assumptions.
6. Model pricing requires the discount curve to exist although the future MTM itself is undiscounted through listed terms. Reported futures price uses the live/settlement mark policy; implied-forward and convexity metrics independently calculate model quantities.

### Risk conventions and selection

When a live quoted mark exists, DV01 clears the quote on a clone if the coarse eligibility predicate sees an explicit convexity adjustment, any supplied vol-surface ID, or a non-Simple compounding mode. It then computes model value and combined or triangular curve risk. The predicate does not verify HW parameters, curve/surface availability or quote conventions; clearing a usable quote can therefore expose missing/invalid model data, and errors propagate. With no live quote or a false predicate the original instrument/value path remains, which can be insensitive to curves. This distinction is observable and must be disclosed with the result.

An IR future option delegates option pricing/Greeks to shared `FutureOptionTerms`. Its rate DV01 is `cash_delta*underlying_price_change_per_bp` plus, for PremiumPaid options, `-T*1bp*NPV` to the relevant cash-payment/expiry discount horizon. Future delivery after exercise has no premium-discount term; futures-style options have none. The price-per-bp conversion is mandatory (typically -0.01 for a 100-minus-rate quote) and supplied by the caller. This is a delta-plus-discount approximation, not full curve repricing.

### Alternatives and tradeoffs

Explicit convexity is supported and can encode an externally calibrated adjustment. The normal-vol term approximation is cheaper and differently parameterized than the overnight Gaussian moment engine. A full stochastic futures calibration is unimplemented. Model-based risk on a quoted position is useful but differs from the derivative of a frozen mark.

## Evidence

- [Future implementation](../../../../finstack-quant/valuations/rates/src/ir_future/types.rs), [future risk selection](../../../../finstack-quant/valuations/rates/src/ir_future/metrics.rs), [option rate DV01](../../../../finstack-quant/valuations/rates/src/ir_future_option/types.rs), [option metric registration](../../../../finstack-quant/valuations/rates/src/ir_future_option/metrics.rs).
- [Convexity tests](../../../../finstack-quant/valuations/rates/tests/ir_future/test_convexity.rs), [reference compounding](../../../../finstack-quant/valuations/rates/tests/ir_future/test_reference_compounding.rs), [metrics](../../../../finstack-quant/valuations/rates/tests/ir_future/test_metrics.rs): formula, information-state and risk behavior evidence.
- The existing [futures schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/interest_rate_future.schema.json) and [futures option schema](../../../../finstack-quant/valuations/schemas/instruments/1/rates/interest_rate_future_option.schema.json) carry the current fields. The [shared option instrument adapter](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/instrument_impl.rs) supplies the construction-time rate mapping gate.
- [Hull](../../../REFERENCES.md#hull-options-futures), [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [ARRC guide](../../../REFERENCES.md#arrc-sofr-users-guide). Contract registry/defaults and `ListedFutureTerms` remain separate material dependencies.

### Independent review evidence

- For a Saturday-to-Tuesday reference, use a known Friday rate
  `.036` with contract weight `2/360` and projected Monday
  `.072` at `1/360`. Arithmetic and compound rates are `.048`
  and `.0480048`; remaining quote sensitivities are `1/3` and
  `.3334`. Friday's full quote still runs to Monday. Incorrectly
  splitting the weekend gives compound rate `.04800600024`.
- With Ho–Lee `σ=.01`, true term fixing `f=1`,
  `S=1.25,E=1.5,τ=.25,F=.04` and origin zero, `z=.000025`
  and the adjustment is `1.0100126251` bp. Substituting `S`
  for the fixing gives `1.1047026033` bp.
- In a synthetic two-block Gaussian fixture with starts `1,1.25`,
  ends `1.25,1.5`, accruals and full weights `.25,.25`,
  forwards `.04,.05`, zero mean reversion and `σ=.02`, the
  compound correction is `4.2826905160` bp, versus
  `3.7712773414` bp without covariance. If the second weight is
  `.125` with full index accrual `.25`, the arithmetic
  correction is `3.4967190043` bp. A two-term exact expansion
  for the clipped compound example gives `3.8651648866` bp;
  incorrectly multiplying all factors by the full-block moment
  gives `5.6750055397` bp.
- Long two contracts at `2,500` per point, current `96` and
  prior clearing `95.99` gives `50` live variation; from fill
  `95.90` the cumulative difference is `500`. A positive
  benchmark bp maps to `−.01` points or `−50` cash. For
  a distinct model experiment with forward `.04`, source time
  and accrual both `.25`, and explicit zero convexity, a central
  ±1 bp curve shock yields DV01 about `−50.5000000053`. A
  held observed mark has zero live curve exposure.
- An ATM premium-paid Bachelier call on a futures price of
  `98.5` with sigma `1.2` points, option time `.5`, factor
  `.98` and long two contracts at `2,500` per point has
  point delta `2,450` and mapped sensitivity `−24.50` at
  `−.01` points per bp. Its gross PV is about `1,658.71737563`.
  With payment at source span `.75`, discount DV01 is about
  `−.1244038033` per bp. The `.5` option-time shortcut gives
  only `−.0829358688`.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
