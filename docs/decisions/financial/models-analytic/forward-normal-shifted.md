# MODELS-ANALYTIC-02: Forward Black, normal and shifted pricing and sensitivities

**Decision status:** Accepted — complete unit-forward Black, normal and shifted pricing/Greek package, 2026-11-09.

**Implementation status:** Observed; reconciliation pending in the source scope and actual product/calibration/host/UI consumers below.

**Validation status:** Production evidence inspected/located, not executed. Independent bounded Decimal/scalar/derivative references and read-only numerical, financial, primary-source and consumer reviews are reported below; they do not certify production accuracy, a complete product or a release.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Observed scope and input domain

European forward calls/puts; Black requires F,K>0, normal permits finite negative F,K, shifted Black requires F+s,K+s>0. Volatility and time are finite and nonnegative.

## Accepted decision: unit forward prices and native model Greek bundles

Accepted on **2026-11-09** after complete whole-record review. Runtime/caller/host
reconciliation and independent complete-path qualification remain pending. Inherit accepted
[01's checked/numerical/whole-bundle contract](spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results),
[market14's actual quote conventions and signed displacement](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks)
and [explicit financial clocks](../conventions/0045-financial-clocks-and-date-boundaries.md).
This record owns scalar forward pricing and the three-field ForwardGreeks bundle.
Product numeraires, cashflows, risk chains and inverse03's solver remain with their
owners. No runtime changes are authorized here.

1. **Select the three forward laws explicitly.** Black76 models a positive
   lognormal forward, Bachelier an arithmetic normal forward, and shifted Black
   a positive lognormal F+s under a fixed displacement. They are European
   call/put scalar models, not automatically selected fallbacks for negative
   rates or a full futures/forward, payment-delay, stochastic-annuity or
   settlement model. The product owner qualifies the payoff and pricing measure.

2. **Use one unit-numeraire scale for every canonical price and Greek.**
   black76_price, bachelier_price and black_shifted_price return unit forward
   premiums U in the underlying F/K units. Their Greek bundles differentiate
   that SAME U. Remove df from Black76 price and the shared Black76 request;
   remove bachelier_price_with_annuity and its unchecked multiplication. Actual
   Rust owners apply the required discount OR annuity and notional once. Keep no parallel scaled/
   unscaled API, inferred multiplier or hidden normalization default.

3. **Validate the complete model domain before all shortcuts.** All supplied
   values are finite. Black requires F,K>0; normal permits either sign and zero.
   Require T>=0 and the model volatility>=0. For shifted Black allow finite
   SIGNED s and require mathematical G=F+s, H=K+s to lie in(0,MAX] within the
   accepted finite transformed-coordinate domain. Evaluate these sums from the
   represented inputs without requiring exact one-f64 representability or
   accepting a rounded contract as the original. Negative time/volatility,
   unknown model/quote assumptions and invalid shifts error even at T0/sigma0.

4. **Keep volatility, displacement and model-time units literal.** Black and
   shifted sigma are relative decimal volatility per sqrt(model year); normal
   sigma_N is in the actual F units per sqrt(model year). On a decimal-rate
   underlying75bp normal volatility is0.0075, not0.75 or a relative75bp return.
   s uses the same F/K units. T is the owner's compatible variance coordinate,
   not an inferred ACT/365F, coupon day count or payment-date discount clock.
   No quote retagging or automatic normal/lognormal conversion is performed.

5. **Define exact zero-time/zero-volatility prices in original coordinates.**
   After3, T=0 or volatility=0 returns max(F−K,0) for a call or max(K−F,0) for
   a put, on unit scale. Shift cancels from the payoff. Preserve original signed
   differences with scaled arithmetic where needed; do not subtract separately
   rounded translated values. Genuine OTM/equal payoff0 is valid. Do not use
   epsilon time/volatility, negative-time clamping or a proxy minimum variance.
   Mathematical expiry payoff does not decide actual fixing/exercise/payment.

6. **Retain the Black and shifted-Black price formula.** In the regular domain
   set G=F,H=K for Black, or G=F+s,H=K+s for shifted Black. Let w=sigma*sqrt(T),
   m=ln(G/H), d1=m/w+w/2 and d2=m/w−w/2. Then
   U_call=G*Phi(d1)−H*Phi(d2),
   U_put=H*Phi(−d2)−G*Phi(−d1). This defines the model, not a requirement to
   evaluate those subtractive expressions literally. The shift is fixed and
   remains part of the quote/model identity.

7. **Retain the normal price formula with signed coordinates.** Let x=F−K,
   u=sigma_N*sqrt(T), d=x/u. In the regular domain
   U_call=x*Phi(d)+u*phi(d),
   U_put=−x*Phi(−d)+u*phi(d). Normal support is unbounded; F/K need not be
   positive and no Black upper-price bound applies. Large x/u arithmetic must
   preserve requested values or error rather than substituting another law.

8. **Define three native forward partials on the same unit scale.** Hold K,T,
   volatility and any shift fixed for forward derivatives; hold the other
   parameters fixed for vega. Use the following regular formulas:

   | Field | Black/shifted Black | Normal |
   | --- | --- | --- |
   | Call delta = U_F | Phi(d1) | Phi(d) |
   | Put delta = U_F | −Phi(−d1) | −Phi(−d) |
   | Gamma = U_FF | phi(d1)/(G*w) | phi(d)/u |
   | Vega = U_sigma or U_sigma_N | G*sqrt(T)*phi(d1) | sqrt(T)*phi(d) |

   Vega is per1.0 of the actual model volatility parameter. Relative-volatility
   percentage points and normal-volatility native-unit/bp conversion are distinct
   reporting operations. This bundle adds no spot delta, theta, rho, shift
   derivative, smile reselection or curve sensitivity.

9. **Use whole-bundle singular availability consistently with01.** Each
   canonical forward-Greek function returns Result<Option<ForwardGreeks>>.
   After complete validation, None means valid T=0 or model volatility=0.
   Regular requests return all delta/gamma/vega or a computation error; invalid
   inputs and numerical failure do not become None. Remove fabricated boundary
   delta1/half-delta and zero gamma/vega, thresholds and individual optional fields.
   Some individual/right derivatives exist, including positive ATM right vega
   at zero volatility; bundle absence is not a claim that these derivatives are0.

10. **Preserve the mathematical displacement, not a rounded replacement.**
    Retain original F,K,s through shifted admission, moneyness, boundary price
    and parity. Distinct F/K can become equal after naive f64 addition of a
    large s. Preserve the difference using checked two-component/scaled/log1p
    geometry or refuse unresolved precision loss; do not infer ATM from equality
    of rounded G/H. Ordinary nonexact additions are not rejected merely for
    needing more than one f64 component. Changing s while retaining sigma
    changes the law/support and is not a price-preserving quote conversion.

11. **Use stable geometry in each law.** Share01's stable log-ratio/total-
    standard-deviation Black primitives; avoid F/K overflow, w² and d2=d1−w
    cancellation. Normal arithmetic must handle signed differences and products
    by scale rather than require unrequested x or u to fit independently.
    Preserve small variance/ATM information; do not make a rounded-zero standard
    deviation a zero-volatility input. Use checked existing Gaussian/erf/log
    primitives and small private financial helpers, with explicit computation
    refusal if the required geometry cannot be resolved. No new precision engine.

12. **Protect requested unit premiums without floors or silent tail loss.**
    Evaluate the OTM side with stable same-function tails/positive differences,
    then use original-coordinate intrinsic/parity for the other side. For normal
    OTM tails preserve phi(z)−z*Phi(−z), including when separate probabilities
    underflow before a large u restores the unit price. ATM limits must preserve
    their complete scale. Require finite nonnegative requested output; known-
    positive regular prices lost to0 or unresolved cancellation error, with
    exact0 only from5. Do not max(price,0), clip a bound, return NaN/reference
    values or require the unrequested opposite option/Greek bundle to fit.

13. **State the deliberate unit-output range boundary.** U is the requested
    output of these APIs and must itself be representable. A later small discount
    factor or annuity cannot rescue an unrepresentable U through this interface;
    do not claim arbitrary final-PV range or add a scaled fallback API. This
    differs from avoiding unrequested intermediate overflow when the requested
    UNIT price is representable, as required by11–12. Price-only calls remain
    independent of Greek availability and numerical range.

14. **Evaluate the requested Greek bundle in its final unit scale.** Use scaled
    density/tail products and denominators, avoiding early probability underflow
    or overflowing G*w. Require finite delta/gamma/vega, nonzero signed delta,
    |delta|<=1 and positive gamma/vega in the regular domain. A CDF may round to
    its correct endpoint1; losing a mathematically nonzero requested Greek to0
    errors. If another component cannot be represented, the whole bundle fails
    even if a single vega would fit. Do not fabricate a scalar Greek, default
    weight or missing bundle to extend the selected API's domain.

15. **Preserve model identities and their actual bounds.** Unit call–put parity
    is U_call−U_put=F−K for all three laws, using the SAME parameters. Intrinsic
    is the lower bound. Black upper bounds are F for call and K for put; shifted
    bounds use F+s/K+s, not possibly negative unshifted levels. Normal has no
    universal F/K upper cap. Normal is invariant to a common additive F/K
    translation. These are mathematical/qualification identities, not permission
    to clip numerical violations or promise exact binary64 equality.

16. **Expose one checked price/bundle pair per law.** Retain black76_price/
    black76_greeks and bachelier_price/bachelier_greeks; retain shifted price and
    replace its vega-only wrapper by black_shifted_greeks returning the same
    three-field bundle. Remove public NaN-returning call/put/individual-Greek
    leaves and annuity siblings rather than preserving raw/checked families.
    Keep needed existing d1_d2_black76 only as checked fallible regular-domain
    geometry. Private shared admitted primitives can support internal numerical
    owners without a parallel public API or duplicate formula implementation.

17. **Guard input and computed-result state.** ForwardGreeks is immutable with
    read-only get_* inspection and no fabricated Default/mutable bypass.
    Construction and restore require finite fields, nonzero |delta|<=1 and
    gamma/vega>0, matching the regular returned bundle. Validate the actual model
    inputs/quote convention before calls and all state fields/duplicate keys
    before collapse; reject unknown fields. Valid shape does not certify the
    model inputs or derivative accuracy. No implicit Money/currency precision
    follows from a numeric premium or result.

18. **Align shared calculation and inverse transport to unit scale.** Existing
    Black76 and normal option_calculation branches return unit price/target
    price and unit Greek bundles, with01's vol name and nullable whole bundle.
    Remove the Black76 df input/field, not an ignored or defaulted alias. The
    Black inverse delegation uses explicit unit normalization (df1 where the
    inverse still takes df), while03 retains bounds, root/initialization,
    residual and singular-inversion choices. An old discounted target cannot
    silently become a unit target; the qualified owner must normalize it first.

19. **Keep product PV/annuity and risk scaling explicit.** Owners select actual
    DF or annuity and notional from their payoff/measure/payment conventions,
    applying each once to U. For decimal-rate swaptions with per-notional
    A=sum(accrual*DF), PV=N*A*U; a genuine one-bp coupon PV is N*A*1e−4, so
    replacing total annuity N*A by its genuine one-bp PV underprices by10,000.
    Recovering per-notional A from that PV requires division by N*1e−4 for
    nonzero N; do not mix notional-inclusive and per-notional multipliers.
    Actual positive source factors may
    exceed1; position/pay-receive signs remain separate. At fixed M, derivatives
    of M*U are M times these partials; varying M/F/vol/shift requires the actual
    product chain rule. Unit Greeks are not curve DV01 or smile-following risk.

20. **Reconcile the actual pricing/calibration consumers together.** Update cap/
    floor, swaption, CMS and spread/variance replication, commodity options,
    inflation floors and listed future options to the checked unit kernels,
    preserving owned multiplier/contract scaling exactly once. Remove private
    tiny-volatility-to-intrinsic and negative-time clamps from canonical model
    calculation; owners handle actual known fixing/expiry selection explicitly.
    Reconcile SABR/Hull–White weights and IV vega use with checked bundle or
    shared admitted internal primitives, preserving real errors instead of
    NaN/zero/default1. Fallible callbacks propagate failures. Product weighting,
    floors, solver, exercise and settlement laws retain their own review.
    In particular, route range-accrual's local Black formula and stddev<1e−6
    intrinsic shortcut through canonical checked unit pricing; its separate
    call-spread widths, clamps and eligibility retain the product owner.

21. **Update hosts, schema and live calculator consistently.** Propagate unit
    prices/targets, removed df/annuity/scalar wrappers, complete shifted bundle,
    nullable Greek results, exact names/getters and strict errors through Rust
    re-exports, Python/WASM, declarations, parity, schemas, examples and UI.
    Label forward results as unit premiums and native derivatives; remove the
    Black-only DF control and stale PV/Greek labels. Existing Rust product
    owners perform financial normalization; bindings/UI do conversion and
    presentation, with no host-only finance or compatibility layer.

22. **Qualify the complete contract independently.** Check all three regular
    laws, call/put/native derivatives, parity/bounds and their unit conventions,
    exact boundaries, normal signed/extreme scales, shifted-coordinate loss,
    stable tails, unit-versus-final-PV range and full-bundle failures. Verify
    actual source multipliers, per-bp/pp report conversions, state/hosts and
    composite/inverse normalization separately. Primary implementation formulas
    and independent bounded references support definitions, not adoption of
    external thresholds/floors or universal ULP/product-price accuracy. Runtime
    and independent complete-path qualification remain pending; documentation
    changes here implement none of these policies.

## Independent bounded reference checks

Independent Decimal/high-precision series and binary64 expressions on2026-11-09,
without importing production code, checked:

- Unit Black F=K100,sigma0.2,T1 has price about7.9655674554, delta about0.53982784,
  gamma about0.01984762737 and raw vega about39.69525475. Multiplying price and
  all fixed-factor partials by0.95 differs from the observed price-only scaling.
  Normal F=K0,sigma_N1,T1 has price/vega1/sqrt(2*pi)>0 and delta0.5; Black's
  F/K upper bounds cannot be imposed on that normal price.
- At sigma0 or T0, Black/normal/shifted unit payoff is original intrinsic;
  the full Greek bundle is unavailable, not delta1/half-delta with zero gamma/
  vega. A right-volatility ATM derivative can still be positive, consistent
  with this deliberately whole-bundle policy.
- F1,K0,s2^54 gives distinct mathematical shifted coordinates but identical
  rounded doubles. Deterministic call remains1. At sigma2^-54,T1, the shifted
  call is approximately1.08331547, whereas treating the rounded coordinates as
  ATM gives approximately0.39894228. Ordinary nonexact additions still qualify;
  exact one-f64 shift representability is not an input requirement.
- Normal F=K0,sigma_N2^1023,T4 has overflowing unrequested standard deviation,
  yet unit price approximately7.1717579868e307 and gamma approximately
  2.2191900979e−309 are representable. A large signed difference can likewise
  overflow while a requested OTM price fits. Deep normal tails may require
  preserving density/probability before large unit-volatility scaling.
- On the SAME notional basis, a genuine one-bp coupon PV is1e−4 of the
  corresponding total annuity N*A. Substituting it for N*A underprices by10,000;
  conversion to per-notional A also removes the included notional.
  Conversely, an unrepresentable requested unit intrinsic is an error here even
  if later multiplying it by a small external factor would fit.
- Ordinary signed-normal and shifted examples, regular finite-difference delta/
  gamma/vega, original-coordinate parity, normal common-translation invariance
  and owner multiplier/quote-unit conversions were checked independently.
  These are bounded references, not production/calibration tests or complete
  product-price/Greek certification.

## Observed choice and rationale

Black uses lognormal forward dynamics, Bachelier arithmetic forward dynamics; shifted Black translates both F and K by a fixed displacement. These are explicit variants, not automatic negative-rate fallbacks.

## Observed algorithm

Black uses d1=[ln(F/K)+σ²T/2]/(σ√T), d2=d1−σ√T and F N(d1)−K N(d2). Normal uses d=(F−K)/(σN√T), C=(F−K)N(d)+σN√T φ(d). Puts use direct negative-tail expressions. Zero vol/time returns intrinsic. Delta is N(d1) or N(d), gamma φ(d1)/(Fσ√T) or φ(d)/(σN√T), vega F√Tφ(d1) or √Tφ(d). At zero vol/time delta(call)=1 when F≥K, put delta=call−1, gamma/vega=0. Shifted price and vega call the Black kernels on translated coordinates.

## Observed selection and controls

`black76_price` multiplies by caller df>0; `bachelier_price` and shifted price remain undiscounted. `bachelier_price_with_annuity` applies the supplied multiplier without separate annuity validation. `black76_greeks` and `bachelier_greeks` return unit, undiscounted forward Greeks. Raw leaf functions return NaN on invalid domain; checked wrappers return Result errors.

## Observed conventions

Lognormal σ is relative annual volatility; normal σN has forward units per √year. Vega is per unit volatility (not divided by 100). A shifted quote must retain displacement metadata and must not be interpreted as ordinary Black on negative inputs.

## Observed limitations, failure and fallback

No automatically chosen shift, annuity, discount curve or sensitivity to the shift is provided. Deterministic ATM deltas are conventions, not differentiability claims. Extremely scaled coordinates can overflow despite finite individual inputs.

## Alternatives and tradeoffs

Normal and shifted Black both support negative unshifted rates, with different tail and unit conventions. Translation retains lognormal upper/lower support constraints; normal is unbounded. Adoption by product remains outside these standalone formulas.

The accepted package chooses common unit premiums and native whole Greek bundles,
with explicit output-range limits instead of adding a generic scale parameter.
Actual product multipliers stay outside the model. Stable checked formulas and
original displacement identity replace public raw/NaN and singular substitutes;
forward/smile conversion, IV and product valuation laws retain their own owners.

## Implementation and validation evidence

[black.rs](../../../../finstack-quant/models/src/closed_form/black.rs), [bachelier.rs](../../../../finstack-quant/models/src/closed_form/bachelier.rs), [forward.rs](../../../../finstack-quant/models/src/closed_form/forward.rs). Inline parity, positive-vega and zero-vol boundary tests establish identities; `black76_price_matches_discounted_leaf_formula` is a production-kernel comparison. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) contains independent price fixtures. References: [Black 1976](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900).

- [Official forward-model formulas and numeraire comparison](../../../REFERENCES.md#forward-option-formulas-and-numeraire) corroborate regular formulas, fixed-shift translation and actual annuity units. External numerical thresholds, limit substitutes and floors are not selected here. Historical Black/Bachelier entries remain bibliographic; no unverified historical equation/page attribution is made.
- Actual [shared calculator](../../../../finstack-quant/models/src/closed_form/calculation.rs), [normal cap/floor](../../../../finstack-quant/valuations/rates/src/cap_floor/pricing/normal.rs), [listed future option](../../../../finstack-quant/valuations/core/src/instruments/common_impl/listed/future_option/types.rs) and [range accrual](../../../../finstack-quant/valuations/exotics/src/exotics/range_accrual/pricer.rs) show scale and duplicate-formula reconciliation boundaries. Independent read-only mathematical, numerical, product/clock and API/primary reviews challenged the whole proposal; no production kernel, product, host or calibration suite was executed.

## Relationship to the accepted quote-grid boundary

[CORE-MARKET-14's complete accepted package](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) retains relative Black
sigma versus Normal sigma in actual underlying units, explicit displacement
and literal source coordinates. Black/shifted-Black adapters require finite
positive transformed forward/strike coordinates; Normal retains finite signed
inputs. Attaching shifts cannot retag unchanged quotes as a price-preserving
conversion. Keep actual zero-time/zero-vol pricing branches at this model owner,
without fabricated implied-vol grid nodes. Record14 was accepted on
**2026-11-08**, with reconciliation/qualification pending; this record's
complete formula/limit package above was accepted separately as MODELS-ANALYTIC-02
on2026-11-09; record14 does not independently adopt its model laws.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
