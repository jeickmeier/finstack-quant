# MODELS-ANALYTIC-15: Checked SABR smile, ATM delta proxy and finite-grid repair

**Decision status:** Accepted — complete explicit SABR smile, checked ATM delta and greatest down-only call-grid repair package, 2026-11-10.

**Implementation status:** Observed. Runtime, direct host, schema and cube-slice UI reconciliation remain open.

**Validation status:** Source, primary references and existing tests inspected. Standalone delta, exact-grid and projection checks cover selected boundaries. Production arithmetic, implied-volatility round trip and complete-path qualification remain open.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked SABR smile and finite-grid repair

The complete 25-decision package was accepted on 2026-11-10.

1. **Bind an explicit convention at smile construction.** Keep one
   immutable runtime `SabrSmile` with checked parameters, finite F,
   positive modeled T and required Normal, Black or shifted Black
   `VolatilityConvention`. Apply
   [record 13](sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes)
   parameter, forward and exact displacement compatibility. Retain
   original F, T and source shift. The quote convention is fixed for
   the smile's lifetime. Keep the smile runtime-only and retire unchecked
   public construction.

2. **Use the same checked quote for ATM and strike evaluation.** ATM
   evaluates the original K=F. Strike and bulk quote operations
   use the bound convention and full record 13 domain. Bulk quotes
   preserve caller order and return atomically. The ordered-grid
   requirements below apply to diagnostics and repair.

3. **Use a checked convention-specific display axis.** For Normal
   use the native strike offset K−F, including signed inputs. For
   Black and shifted Black use log((K+s)/(F+s)), with s=0 for
   unshifted Black. Resolve the original coordinate and shift in
   Rust. Remove the NaN moneyness sentinel. Python's DataFrame
   formatter displays `strike_offset` for Normal or `log_moneyness`
   for Black and shifted Black and propagates errors.

4. **Name and define the ATM delta proxy.** Keep one
   `strike_from_atm_delta` operation with an explicit call/put,
   finite absolute delta in (0,1), and finite relative
   `delta_rel_tol` in (0,1). The target is the magnitude of
   premium-unadjusted unit-forward delta with volatility fixed
   at the checked ATM quote. It is a strike convention. The
   selected method does not solve a strike-local smile delta
   equation or a SABR hedge derivative.

5. **Use the analytic strike for the selected proxy.** Let eta=+1
   for call and −1 for put, w=sigma_ATM sqrt(T), and z=Phi^-1(delta)
   from the accepted core quantile. Normal uses K=F−eta w z.
   Black and shifted Black use x=F+s, e=w(w/2−eta z) and
   K=x exp(e)−s. A 50-delta Black strike uses e=w²/2.
   Apply the shift once in the Black coordinates.

6. **Restore the actual strike from original geometry.** Use
   checked scaled products, differences and exponentials. Use
   F+x expm1(e) near equal levels and preserve x exp(e) in scale
   when it is small or large. Require a finite represented K and
   validate its record 13 backbone and output coordinate domain. A
   Normal proxy can generate a signed strike outside a positive
   SABR backbone. No strike clipping or alternate proxy.

7. **Recheck the smaller delta tail at the returned strike.**
   Recompute d=(F−K)/w for Normal or
   d=log((F+s)/(K+s))/w+w/2 for Black, using the actual K.
   Set p=min(delta,1−delta). Evaluate log Phi(eta d) when
   delta<=1/2 and log Phi(−eta d) otherwise, using the
   [accepted core normal functions](../core-numerics/distribution-functions.md).
   Require the difference from log p within the inclusive
   log1p(−delta_rel_tol), log1p(delta_rel_tol) interval.
   Propagate failed geometry, precision or tail checks.

8. **Validate the complete sampled grid.** Require at least two
   finite original strikes in strictly ascending order and in
   the selected quote domain. Require finite nonnegative
   `price_abs_tol` in unit premium. Validate the shape and
   controls before pricing the grid. Preserve original order,
   reject duplicates and use no strike sorting or filling.

9. **Use undiscounted unit call premiums.** Evaluate each checked
   SABR quote and price its call with the selected
   [record 02 forward price](forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles).
   Use original F and K and the active Black displacement.
   Remove r, discounting and hidden absolute thresholds from
   the grid API. Unit premiums use the underlying units.

10. **Include the exact lower-support anchor for relative quotes.**
    For Black prepend (K_0,C_0)=(0,F). For shifted Black use
    (−s,F+s). The anchor is fixed by the quote pricing law and
    precedes all observed strikes. Use exact mathematical
    geometry, including a shifted forward that is in range
    and lacks exact binary64 representation. Do not request
    a SABR quote at the anchor. Normal has no finite anchor,
    including Normal on a displaced SABR backbone.

11. **Check the sampled price bounds.** For each observed strike
    check the lower bound I_i=max(F−K_i,0). For Black and
    shifted Black also check the closed sample upper bound
    U=F+s, with s=0 for unshifted. Normal has no finite upper
    bound. Keep the exclusive Black bound for any changed
    repair target that needs a finite-volatility output.

12. **Check both vertical bounds and actual unequal-gap convexity.**
    For each adjacent pair check 0<=C_i−C_(i+1)<=K_(i+1)−K_i.
    For each adjacent triple let g_L and g_R be its two
    positive strike gaps. Require
    C_i<=(g_R C_(i−1)+g_L C_(i+1))/(g_L+g_R).
    Include the applicable anchor in both checks. Express
    excesses as actual unit-premium differences, including
    the amount above the chord. Remove the factor two and
    percentage severity fallback.

13. **Use one finite-grid tolerance and report coverage.** A
    violation is an exact positive constraint excess greater
    than price_abs_tol. Retain its kind, observed or anchor
    coordinates, and native excess rounded upward for display.
    Compare before display rounding. Report whether the anchor was used
    and how many pairs and triples were checked. Two observed
    Normal strikes have no convexity triple. A pass describes
    these sampled inequalities at the supplied tolerance.

14. **Preserve exact geometric comparisons.** Treat finite
    represented strikes, premiums, forwards, shifts and
    tolerances as exact dyadic inputs. Use exact sums,
    differences and cross-products for ordering, bounds,
    hull decisions and constraint comparisons. Retain an
    interpolated target as its exact chord ratio through
    feasibility and move checks. The existing big-integer
    dependency can express this bounded geometry using integer
    multiples of 2^-1074 for the represented inputs.
    Require checked finite output and allocation sizes.

15. **Select a greatest down-only exact price target.** Repair
    is explicit. Validate all repair and inverse controls before
    generating quotes. Given raw sampled call ceilings C_i, choose
    the componentwise greatest grid at or below them meeting
    the exact bounds, vertical and convexity rules and fixed
    anchor where applicable. The diagnostic tolerance is
    reserved for numerical output checks. A raw sample below
    its exact intrinsic has no feasible down-only target and
    fails repair. Preserve that cause.

16. **Apply two slope-limited ceiling passes.** Include the
    anchor in the input when present. Take
    L_i=min_{j<=i} C_j and R_i=min_{j>=i}(C_j+K_j−K_i).
    Compute L by a forward minimum and R by the backward
    recurrence R_i=min(C_i,R_(i+1)+K_(i+1)−K_i), starting
    from the last raw price. Use M_i=min(L_i,R_i), equivalently
    min_j[C_j+max(K_j−K_i,0)]. This is the greatest
    nonincreasing one-Lipschitz grid
    under the supplied ceilings.

17. **Take the lower convex hull of the slope-limited grid.**
    Stream ordered (K_i,M_i) through a lower hull, dropping
    the middle vertex when two consecutive slopes fail to
    increase strictly. Compare slopes with exact products.
    Evaluate each hull chord at the original enclosed knots.
    The resulting C_i^* is the greatest convex minorant of
    M. Each hull chord has slope in [−1,0]. Intrinsic is
    itself a feasible lower grid, so it is preserved and
    the exact Black anchor stays fixed.

18. **Bound work by the selected passes and inversions.** The
    ceiling passes, hull scan and chord fill use linear grid
    work. Each knot enters and leaves the hull at most once.
    Remove the repair-iteration argument and fixed epsilon
    price reductions. Preflight grid sizes and changed-node
    work using the required record 03 IV controls. Count
    at most one inverse per changed observed node with an interior
    premium. Preserve the accepted inverse's work and failure contract.

19. **Require an explicit native move limit.** Repair takes a
    finite nonnegative `max_abs_reduction` in unit premium.
    Require every exact target reduction C_i−C_i^* within
    the limit. Zero permits exact preservation only. A
    larger required reduction rejects the operation before
    inversions. This cap applies to each node and to the
    actual repriced result.

20. **Restore changed targets under the accepted inverse domain.**
    Preserve the original quote at an exactly unchanged node.
    At a changed Black or shifted Black node check the strict
    mathematical upper bound first. Exact C_i^*=I_i admits zero volatility
    by the checked zero-volatility price identity. For a
    positive time-value target restore its finite nearest
    binary64 premium and require it remain in the actual
    interior. Use the original coordinates and the
    [accepted model inverse and explicit IV controls](implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance).
    A rounded interior target at intrinsic or the Black
    upper bound errors. Retain the actual inverse failure.

21. **Reprice all returned quotes before accepting repair.**
    Use the same checked forward price on the actual returned
    vols. Require each actual repriced call at or below its
    raw call and inside the native move cap. Compare its
    absolute difference from the exact hull target to
    price_abs_tol. Apply the same grid checker to the
    repriced vector. Inversion, display rounding or grid
    failure returns an error with no partial result.

22. **Return an inspectable finite-grid artifact.** Preserve
    strike order, quote convention and active displacement.
    Return repaired vols, exact-target change indicators, target
    premiums with their display rounding, actual repriced premiums
    and native reductions from the raw calls. Include the
    post-repricing report.
    Keep checked immutable outputs. Derive the grid verdict
    from the actual findings. Publish output-only diagnostic
    state with no unchecked restore or independently mutable
    pass flag. Keep one grid checker and one repair path.

23. **Keep failures and the scope of the repair literal.**
    Invalid request fields use validation errors. Unresolved
    arithmetic, an infeasible selected target, failed inverse
    or postcheck preserves its computation or convergence
    cause. A successful result is an opt-in sampled quote
    artifact. It supplies no tail extension, calendar check,
    continuous arbitrage certificate or refit of the original
    SABR state. A positive flat last edge can pass these
    inequalities and still prevent a decaying convex extension.

24. **Reconcile the actual host and cube-node UI routes.** Keep
    the direct Python/WASM smile bound to explicit quote
    meaning. Pass the selected convention and grid/repair
    controls through the existing cube-slice service and UI.
    Label outputs from their actual convention and units.
    Reconcile facade, stubs, generated contracts, schema,
    parity and the SABR notebook. Remove duplicate global
    no-arbitrage wrappers and the old rate/pass inputs.
    Rust owns checks and repair. No calibration, pricing or
    stored cube path is authorized to repair automatically.

25. **Qualify proxy and grid acceptance separately.** Compare
    independent delta, shifted-domain and extreme restoration
    cases. Compare the hull with independent constrained
    grid solutions, including the anchor, irregular strikes,
    vertical excess, lower and upper boundaries, explicit
    move limits and reprice failures. Check both quote and
    realized premium outputs in actual hosts and the UI.
    Keep the finite-grid proof separate from complete smile
    and market qualification.

## Independent bounded reference checks

- With F=−0.002, T=1 and ATM Normal volatility 0.006, the
  25-delta call and put proxy strikes are about 0.002046938501
  and −0.006046938501. With shifted Black at the same F and
  T, s=0.03 and ATM volatility 0.2, the corresponding strikes
  are about 0.002691086617 and −0.005039200852. The 50-delta
  shifted Black strike is about −0.001434362479.
- With exact beta-zero Normal, F=2^54, total standard
  deviation 0.25 and a 25-delta call, the represented strike
  rounds to F and reprices at 50 delta. With shifted Black
  F=0, s=1, total standard deviation 12 and absolute put
  delta 1e−20, the positive effective strike is about
  9.96e−18. The represented original strike −1 loses that
  effective positivity. Both cases need returned-strike checks.
- With Normal beta=0.5, alpha=0.1, nu=rho=0, F=0.001 and T=1,
  the ATM quote is about 0.002174065891. The 25-delta put
  proxy gives K≈−0.000466385160, outside that SABR backbone's
  positive strike domain.
- For Black F=1, strikes [0.9,1] and unit calls [0.5,0.3],
  both nodes pass their price bounds and calls decrease.
  The vertical slope is −2. With the exact anchor, the
  greatest down-only repair is [0.37,0.3], with observed
  slope −0.7.
- For Black F=1, strikes [0.75,1.25] and unit calls [0.4,0.5],
  the selected repair is [0.4,0.4]. Its sampled constraints
  pass. Its positive flat last edge precludes a convex
  decreasing extension with call limit zero.
- Independent exact rational LP vertex enumeration matched the
  selected target on 24 irregular three-observation Normal,
  Black and shifted Black grids. The target dominated every
  feasible vertex componentwise.
- With the anchor (0,1), strikes [0.5,1] and represented
  binary64 raw calls [0.7,0.3], the ideal first target is the
  exact midpoint of 1 and the stored 0.3. Its nearest binary64
  value 0.65 lies about 2.78e−17 above the exact chord. The
  actual repricing must still pass the supplied grid tolerance.
- An independent Black SABR expansion calculation with F=1,
  beta=0.2, alpha=0.4, nu=0.4, rho=−0.7, T=1 and strikes
  [0.1,0.2,0.3,0.45,0.6,0.8,1,1.25,1.6,2] passes the
  observed-node slope checks. The support-anchor butterfly
  fails at K=0.1. Its exact-grid repair lowers that first
  unit call from about 0.903958430899 to 0.903659889692.
  Later calls stay fixed. A standalone Black inverse repriced
  the first repaired target within 1e−12 unit premium.

## Observed scope and input domain

Single-expiry SABR smile with user strike grid and continuous discount rate. Arbitrage grids must be finite and strictly ascending, but empty/singleton grids are accepted.

## Observed choice and rationale

Delta-to-strike uses ATM volatility as a proxy; discrete call prices are checked for monotonicity and convexity, optionally greedily reduced and reinverted. Repair returns a volatility vector and does not refit SABR parameters.

## Observed algorithm

For absolute delta δ, z=N⁻¹(δ); normal call strike=F−σATM√T z, put flips z. Lognormal/shifted K=(F+s)exp[−σATM√T signed_z+σATM²T/2]−s. Generate SABR quotes at each strike and price discounted Bachelier for β≈0 or shifted Black otherwise. Monotonicity fails if Ci>Ci−1+1e−6. Convexity uses unequal-grid linear interpolation of neighbor prices; violation=2(interpolated−Ci)<−1e−6. Repair passes first enforce decreasing prices by Ci=max(Ci−1−1e−8,0), then reduce each interior above its neighbor interpolation. Only modified nodes are inverted through shared IV solvers; all repaired prices are regenerated and revalidated.

## Observed selection and controls

Absolute delta must lie in (0,1). Repair iteration budget is caller-supplied; no hidden retries or alternative optimizer. `check_no_arbitrage` converts diagnostics into an error; `validate_no_arbitrage` returns violations. Severity percentage is butterfly magnitude/middle price×100 when middle price>1e−6.

## Observed conventions

Delta is premium-unadjusted forward delta in the ATM-vol proxy, not the delta of the fitted smile at its own strike. Arbitrage tolerance is absolute price units, not volatility/basis points.

## Observed limitations, failure and fallback

Delta strike is not solved self-consistently against strike-dependent SABR volatility. Greedy repair can move prices below attainable intrinsic, causing inversion failure; remaining arbitrage errors after budget exhaustion. Empty/thin grids passing says nothing about unobserved strikes. No calendar condition or global wing check is performed.

## Alternatives and tradeoffs

Self-consistent strike root finding or constrained convex regression would be alternative unimplemented policies. Existing grid repair is fast and local but not a minimum-distance calibration.

## Implementation and validation evidence

[smile.rs](../../../../finstack-quant/models/src/volatility/sabr/smile.rs) and [tests.rs](../../../../finstack-quant/models/src/volatility/sabr/tests.rs). Uneven-grid, two-strike monotonicity, empty-grid and post-repair revalidation tests exercise discrete mechanics; delta round trips use ATM-vol Black, so do not validate smile-consistent delta. Reference: [SABR](../../../REFERENCES.md#hagan-2002-sabr).

Actual consumers include the [Python](../../../../finstack-quant-py/src/bindings/models/volatility.rs)
and [WASM](../../../../finstack-quant-wasm/src/api/models/volatility.rs)
smiles, [cube-node service](../../../../finstack-quant-ui/registry/shared/file/services/models.ts)
and [cube-slice component](../../../../finstack-quant-ui/registry/models/components/vol-cube-slice/vol-cube-slice.tsx).
See the [delta and discrete-price references](../../../REFERENCES.md#sabr-delta-and-finite-strike-grid).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
