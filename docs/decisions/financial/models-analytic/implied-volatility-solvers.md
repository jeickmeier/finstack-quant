# MODELS-ANALYTIC-03: Explicit model volatility inversion

**Decision status:** Accepted — complete explicit model/controls, bounded inverse and paired premium acceptance package, 2026-11-10.

**Implementation status:** Observed. Reconciliation remains open for the source scope and actual product/calibration/host/UI consumers below.

**Validation status:** Production evidence inspected/located, not executed. Independent bounded references and read-only numerical, financial, primary-source and consumer reviews are reported below; they do not certify production inversion, a complete product or universal volatility accuracy.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Observed scope and input domain

Positive expiry and finite nonnegative premium; Black requires positive forward/strike, normal finite forward/strike. Adapters accept discounted spot or forward prices.

## Accepted decision: explicit bounded model inversion and paired premium acceptance

Accepted on **2026-11-10** after whole-record review. Inherit accepted
[01's checked spot prices and composite result](spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results),
[02's unit forward laws and preserved displacement](forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)
and the accepted [core Brent contract](../core-numerics/brent-root.md).
This decision owns scalar European price-to-volatility inversion. Product owners
retain exercise, settlement, quote construction and calibration acceptance.

1. **Use one canonical model inverse.** Select
   `implied_vol(model: &OptionModel, price: f64, controls: &IvControls) -> Result<f64>`.
   Reuse the existing OptionModel rather than four parameter families. Its
   BlackScholes, Black76 and Bachelier variants use accepted01/02 inputs;
   add ShiftedBlack with forward, strike, shift, expiry and is_call for the
   already supported shifted law. One private solving authority serves all
   four variants. Keep a scalar result, without a new result/status framework.

2. **Keep target and volatility units model-specific and explicit.** Spot BSM
   accepts the original spot premium; Black76, Bachelier and ShiftedBlack accept
   the UNIT forward premium defined by02. Black/shifted IV is relative decimal
   volatility per sqrt(model year); normal IV is in actual underlying units
   per sqrt(model year). The owner supplies compatible T, model/quote identity
   and normalization. Notional, position, DF/annuity and bid/ask treatment are
   actual product/quote operations, applied once outside the forward inverse.

3. **Validate all inputs and require identifiable positive time.** Price must
   be finite and nonnegative, with the complete finite model domain of01/02.
   Require T>0. At T=0 an intrinsic-consistent premium does not identify
   volatility, so error rather than returning0. Reject negative/nonfinite time
   and invalid model/shift/controls before any endpoint shortcut. Signed r/q
   and normal F/K remain valid within their accepted domains.

4. **Use literal model bounds, with the exclusive upper bound checked first.**
   For spot define A=S*exp(-q*T), B=K*exp(-r*T); for Black use A=F,B=K;
   for shifted Black use mathematical A=F+s,B=K+s. Call lower bound is
   L=max(A-B,0), put L=max(B-A,0). Their upper bound U is A for call, B for
   put; require P<U BEFORE testing lower equality. Equality at U requires
   infinite volatility. Normal L is original max(±(F-K),0), with no finite
   upper premium bound. Reject P<L strictly; no epsilon bound repair.

5. **Select zero IV only at exact lower equality.** After complete validation
   and4, literal P=L selects canonical sigma0, including genuine OTM/ATM P0.
   This is the only return path permitted to produce0. Every interior target
   requires finite strictly positive returned sigma, including endpoint,
   computed-zero and loose-tolerance exits. A rounded zero-volatility price
   is not the exact mathematical lower bound. This point-inverse convention
   does not reconstruct the volatility behind a rounded observation.

6. **Preserve original-coordinate bounds and premium information.** Retain
   original S,K,r,q or F,K,s and their exact represented-input geometry.
   Use checked exact/scaled differences and stable log/discount legs rather
   than comparing separately rounded intrinsic/transformed levels. Do not
   permanently replace BSM by rounded discounted S/K, divide by tiny DF, or
   turn shifted sums into a different rounded contract. If a near-bound
   comparison/equality cannot be resolved reliably, return a precision/
   computation error. Lost quote time value cannot be recovered by inversion.

7. **Require one validated five-field IvControls.** All fields are required:
   max_vol finite>0, the annual model-volatility search bound; vol_tol finite>0,
   an absolute annual model-volatility coordinate tolerance; max_iterations
   integer>=1, the core Brent update budget; price_abs_tol finite>=0 in the
   ORIGINAL supplied premium units; and price_rel_tol finite>=0 dimensionless.
   At least one price tolerance must be positive. Black and normal vol_tol
   units differ with the model. No built-in financial adequacy is inferred
   from structurally valid, potentially loose caller tolerances.

8. **Search only the caller's explicit annual-volatility interval.** Use
   [0,max_vol], including its upper endpoint. Remove the fixed1e-16 floor,
   Black ceiling10 and level-dependent normal ceiling. max_vol is a requested
   search bound, not a financial volatility ceiling. An admissible target
   needing more volatility causes a bounded-search failure, not an invalid-
   premium error or a claim that no mathematical IV exists. No guess, automatic
   expansion, clamping, implicit cap or model substitution is selected.

9. **Use the existing supplied-bracket Brent kernel.** Select derivative-free
   Brent on direct annual sigma, with vol_tol and max_iterations mapped to
   accepted core05 controls. Remove the bespoke analytical seed,20 bisections,
   four Halley steps and Newton fallback. Core termination is a computed
   objective zero OR its accepted coordinate-width test; it does not promise
   both, a true-root error bound or financial premium acceptance. vol_tol is
   an operational coordinate control, not a guaranteed IV-error certificate.

10. **Retain the positive OTM target in original premium units.** For an
    interior request define p=P-L>0 using the preserved geometry, not two
    separately rounded totals. For an ITM leg this is the price of the
    opposite OTM leg under parity; for an OTM/ATM request p=P. BSM moneyness
    here uses the discounted A/B legs, not spot S versus K. Preserve p by
    checked exact/scaled positive differences. In the spot model it remains
    discounted spot premium; in forward models it remains unit premium.
    Refuse unresolved cancellation rather than manufacturing p0.

11. **Use a finite bounded log-OTM objective.** For positive same-law OTM
    model price q(sigma), evaluate
    f=(q-p)/(q+p)=tanh((log(q)-log(p))/2). The genuine sigma0 limit is f=-1;
    lower-equality requests were handled by5. This avoids an infinite log0
    endpoint and raw-price overflow. Resolved large log differences may
    saturate the bounded objective at±1. Numerically equal log values or a
    computed f0 do not prove exact inversion; every exit still passes14–15.

12. **Share checked price-only primitives without requiring public outputs
    or Greeks during iteration.** A private same-law log-OTM evaluation must
    preserve01/02 geometry and tails; it is not log(public rounded price).
    Public positive-price underflow or an overflowing trial premium must not
    erase a resolvable log objective. No representable raw vega or complete
    Greek bundle is required. Actual unresolved geometry/log/cancellation or
    source failures propagate; never catch a failed price and substitute a
    synthetic sign,0, NaN or huge residual. Objective saturation needs resolved
    same-law arithmetic, not an inference from a failed public price.

13. **Keep endpoint evaluation, work and failure truthful.** Inherit core05's
    lower-then-upper admission, fallible finite callbacks, sign rules, caching,
    stagnation and error propagation. The analytic lower f=-1 is known for
    interior targets. At most two endpoint evaluations plus max_iterations
    new candidate updates are allowed by the root kernel, with bounded final
    repricing in15 separate. No hidden retries, alternative-root-family
    fallback or private polishing budget is added. The upper endpoint can be
    accepted if all final gates pass; otherwise preserve the actual failure.

14. **Check the actual returned annual volatility on every exit.** Require
    finite sigma in[0,max_vol], and sigma>0 for every interior target. Zero
    belongs only to the exact-lower branch. Validate the actual returned
    representable value, not an unreturned total standard deviation or a
    pre-rounded iterate. Do not clip, floor or return a best failed iterate.

15. **Apply paired final gates in original premium units.** Every path,
    including exact lower, upper endpoint, computed zero and width exit, must
    reprice the ORIGINAL requested model/leg at the actual returned sigma.
    Independently evaluate its same-law OTM q; do not obtain q by subtracting
    rounded total price and intrinsic. Require BOTH
    |V(sigma)-P|<=price_abs_tol+price_rel_tol*P and
    |q(sigma)-p|<=price_abs_tol+price_rel_tol*p.
    At the exact-lower branch p=q=0. Use checked scaled comparisons without
    an overflowing or underflow-collapsed budget granting success. Preserve
    public requested-price errors, including accepted01/02 range limits.
    Remove max(scale*1e-14,target*1e-8), normal1e-10 scale floors and
    normalized-only acceptance. A large intrinsic amount must not hide a bad
    time-value fit; an OTM fit must not excuse bad total-price reconstruction.

16. **State conditioning and accuracy limits honestly.** Locally
    d(sigma)/dP=1/vega in compatible units. Near intrinsic/Black upper bounds,
    low vega and binary64 quote rounding can make volatility poorly resolved.
    Remove the universal machine-precision/<1e-12 relative-IV claim. Paired
    computed-price gates and core stopping controls do not bound kernel/model/
    source error or recover latent generating IV, and do not guarantee nearest
    rounding, uniform volatility digits or economically adequate tolerances.

17. **Freeze one coherent input request throughout solving.** The model,
    premium normalization, shift, clock and resolved source assumptions stay
    fixed for all callbacks and final checks. No live market requery, curve
    rebinding, quote retagging or automatic model choice occurs in this scalar
    inverse. Actual dated discount/forward/annuity/measure ownership remains
    with the caller and accepted market/convention records.

18. **Validate controls/state and preserve native errors.** IvControls is a
    public immutable validated Rust value with private fields and read-only
    get_* accessors. No Default, implicit JSON/host defaults, public mutation
    or restore bypass; reject unknown and duplicate fields before collapse.
    Host parameter objects use canonical Rust validation, without a second
    host state class. Invalid inputs/bounds/unidentifiable time are validation
    failures; insufficient bracket, exhausted work, stagnation, unresolved
    precision and failed final gates are computation/solver failures. Preserve
    existing error kinds and source context; VolatilityConversionFailed maps
    to core Computation/Python RuntimeError, not ValueError. Valid shape does
    not establish financial adequacy or numerical accuracy.

19. **Align the composite and all four model branches.** Require controls in
    OptionSolve::ImpliedVol {price,controls}; Price {vol} carries no unused
    solver controls. Route ShiftedBlack price, inverse and full Greek bundle
    through02's existing law. Keep01's result.price target and result.vol
    meaning, whole Greek bundle None for solved sigma0, and composite failure
    if regular Greek computation fails. Direct implied_vol remains independent
    of full-Greek range; no hidden partial-result mode is added.

20. **Remove obsolete public inverse routes.** Remove bs_implied_vol,
    black76_implied_vol and raw volatility::implied_vol_black/
    implied_vol_bachelier aliases, their discounted-Black df argument and
    obsolete exports rather than maintaining parallel signatures or
    compatibility wrappers. Update Rust re-exports and direct callers to1.
    Do not introduce four new model-specific parameter structs or a second
    private formula/solver implementation.

21. **Preserve actual product-specific inverse objectives.** Equity exercise/
    discrete-dividend repricers, swaption/cap-floor/CDS-option objectives,
    convertible yields/volatility and Hull–White portfolio/report inverses
    retain their qualified product repricing and quote basis. They are not
    replaced by European scalar BSM just because the output is called IV.
    Reconcile their errors-to-NaN, expiry-zero, floors/caps and final residual
    paths explicitly at their owner; the relevant owners retain those product laws, calibration weighting and
    plausibility choices.

22. **Reconcile the actual shared-inverse consumers.** Update SABR smile
    repair's discount/normal/shifted routing and LMM calibration's displaced
    price matching with original F,K,s and correct units. Preserve actual
    inversion failures instead of LMM/Rough-Heston Option/.ok() erasure.
    Reconcile FX's expired-IV0 sentinel and notional-only normalization of
    its spot premium; no extra DF is applied. Actual cap-floor/ATM quote
    conversions retain their owner objectives and require separate comparison
    of solved quotes, source prices and acceptance. No consumer-generated
    target or calibration round trip proves independent model accuracy.

23. **Update hosts, wire and live calculator as one contract.** Propagate
    canonical model/name, ShiftedBlack transport, required five controls,
    removed aliases/df fields, units and truthful errors through Python/WASM
    exports/stubs/declarations, schemas, parity, examples and UI. Present model
    volatility and native premium/tolerance units explicitly. Existing Rust
    owners perform finance and solving; hosts/UI convert/present with no
    inferred controls, host solver or silent source/default normalization.

24. **Qualify bounds, inverse acceptance and complete consumers separately.**
    Independently check exact endpoints/upper-first order, nonidentifiable T0,
    normal translation/scaling, shifted original geometry, high/tiny IV,
    near-bound conditioning, caller range/work limits, private-log failures and
    EVERY final gate/exit. Compare independent prices/inverses with explicit
    total-standard-deviation versus annual-volatility conversion. Primary
    implementations support coordinates/conditioning, not inheritance of
    external caps/epsilon shortcuts. Production, strict state/host/UI and
    complete product/calibration qualification remain pending. Numerical
    success certifies neither quote suitability nor an arbitrage-free smile.

## Independent bounded reference checks

Independent Fraction/Decimal/scalar references on2026-11-09, without production
imports, checked:

- Black F1,K2^-54 has exact L=1-2^-54, which rounds to1. Target P1 is the
  EXACT exclusive upper bound, not sigma0; next_down(1)=1-2^-53 is below L
  by2^-54 and invalid. No binary64 premium lies strictly between these bounds.
  Rounded-intrinsic equality cannot implement this mathematical point inverse.
- Normal ATM F=K0 versus F=K2^60, P=T1, has identical
  sigma_N=sqrt(2*pi)=2.5066282746310005024... . The observed level-dependent
  epsilon can falsely return0 after translation. At F=K0,T1,sigma_N20,
  price=7.9788456080286535588...; exact target P8 implies sigma_N about
  20.053026197048004, outside the observed ceiling10 despite a valid model
  price. A caller max_vol10 would instead give a truthful bounded-search error.
- Black ATM F=K=T1,P=1-2^-53 has finite
  IV=16.584722151627191076468304627558578854... and raw vega about
  4.668308725781713e-16. Independent Machin-pi/Gaussian-integral series and266
  bisections at150 and180 Decimal digits agreed through this displayed IV.
  This is one bounded reference, not a proof of those digits in production.
- Black ATM P2^-100,T1 has IV between L0 and L0*(1+2e-61), where
  L0=sqrt(2*pi)*2^-100 is about1.977381049777994e-30. Direct independent
  small-argument series confirm positive IV far below the observed1e-16 floor.
- Normal F2^54,K1,P2^54,T1 has exact intrinsic18014398509481983 and positive
  OTM target p1, although rounded intrinsic equals P. For a candidate with OTM
  q2 and abs0/rel1e-8 tolerances, original-premium error1 passes its budget
  about180143985, while OTM error1 fails budget1e-8. The bounded objective is
  1/3. This checks the paired gates' distinct information, not a solved candidate.

These exact/series references qualify particular mathematical distinctions,
not the production solver, host behavior or domain-wide accuracy.

## Observed choice and rationale

Price inversion uses parity to move to the OTM side, an analytical seed, bracketing/bisection, then Halley refinement with Newton fallback. The implementation is not a generic unlimited implied-vol solver.

## Observed algorithm

Check intrinsic bounds; raw Black returns zero at intrinsic and rejects prices at its finite upper bound. Raw normal returns zero within machine-epsilon scaled intrinsic. Transform ITM premium to OTM by subtracting intrinsic. Black seed blends √(2π/T) premium/F with √(2|ln(F/K)|/T), weight=min(5|ln(F/K)|,1). Normal seed is premium√(2π/T). Double the upper bracket up to 64 times, then 20 bisections. Perform up to four Halley steps σ−2fv/(2v²−f volga); use Newton when denominator is too small, stop when vega underflows or step<1e−14. Clamp iterates. Reprice and require residual≤max(target×1e−8,scale×1e−14).

## Observed selection and controls

Vol floor=1e−16; Black ceiling=10; normal ceiling=10 max(|F|,|K|,1). BS adapter maps price to Black with discounted S and K; Black76 adapter uses dfF and dfK, avoiding dividing by tiny df. Those adapters require strictly positive premium above discounted intrinsic, so they reject the zero-IV solution raw solvers accept. Failures return explicit conversion/validation errors.

## Observed conventions

Time is years; returned Black IV is decimal annual volatility, normal IV in underlying units/√year. Adapters preserve price scaling; the normal scale floor is 1e−10.

## Observed limitations, failure and fallback

Ceilings can reject valid very-high-volatility inputs. Bracket expansion has a fixed work budget, and final residual—not termination alone—governs acceptance. Near intrinsic a zero-volatility answer is convention-dependent and differs between entry points.

## Alternatives and tradeoffs

The observed hybrid has small fixed work but inconsistent boundary policies,
unit-dependent caps and no uniform IV-accuracy guarantee. The accepted method reuses
the accepted core solver with explicit controls and financial acceptance;
a new specialized rational solver, public tuning framework and fallback family
are not selected. Stable OTM geometry can preserve information still in the
inputs, but cannot recover premium information already rounded away.

## Implementation and validation evidence

[implied.rs](../../../../finstack-quant/models/src/volatility/implied.rs) and [implied_vol.rs](../../../../finstack-quant/models/src/closed_form/implied_vol.rs). `solver_only_returns_ok_for_genuinely_converged_solutions` reprices accepted inversions and `sub_intrinsic_prices_are_rejected` checks bounds. [vol_models_quantlib_tests.rs](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) uses external premiums plus IV round trips; round trips primarily test inversion consistency. References: [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900). No canonical reference for this precise hybrid/seed/ceiling policy was established.

Observed constants and epsilon/intrinsic/OTM branches are in implied.rs; existing
adapters reject intrinsic equality and use rounded discounted coordinates.
[The shared calculator](../../../../finstack-quant/models/src/closed_form/calculation.rs)
has three model variants and an ImpliedVol request containing only price today. Actual callers
include [SABR smile repair](../../../../finstack-quant/models/src/volatility/sabr/smile.rs),
[LMM calibration](../../../../finstack-quant/calibration/src/lmm.rs),
[Rough Heston](../../../../finstack-quant/models/src/volatility/rough_heston.rs)
and [FX premium inversion](../../../../finstack-quant/valuations/fx/src/fx_option/pricer.rs).
[Equity's product inverse](../../../../finstack-quant/valuations/equity/src/equity_option/types.rs)
and [Hull–White cap inversion](../../../../finstack-quant/calibration/src/hull_white/pricing.rs)
illustrate distinct owned repricers, not automatic scalar-model replacements.
[Core error classification](../../../../finstack-quant/core/src/error/mod.rs)
and [Python mapping](../../../../finstack-quant-py/src/errors.rs) determine actual
exception categories. Primary formula/inverse-coordinate comparisons are in
[the reference register](../../../REFERENCES.md#implied-volatility-inversion-and-conditioning).
No production tests, builds or generators were run for this documentation review.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
