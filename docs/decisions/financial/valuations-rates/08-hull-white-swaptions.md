# VALUATIONS-RATES-08-HULL-WHITE-SWAPTIONS: Hull–White swaption exercise, tree reuse and regression policy

- ID: VALUATIONS-RATES-08-HULL-WHITE-SWAPTIONS
- Decision status: **Accepted — complete checked Hull–White swaption exercise and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, numerical, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked Hull-White swaption exercise and risk

1. **Select the engine and one complete dated model block.** Require an
   explicit registered Hull–White tree for European or Bermudan
   exercise, or the explicit Hull–White LSMC model for Bermudan exercise.
   Remove the implicit Bermudan tree choice and reject unsupported engine
   or product combinations. The LMM route remains with
   [record 09](09-lmm-bermudans.md). Bind one explicitly shared typed
   evaluator for projection and discount, on one native currency; equal
   ID text never admits two different evaluators. Require one complete
   authored or exact fitted swaption-family block, with finite
   nonnegative mean reversion per model year and finite nonnegative scalar
   absolute short-rate sigma per square root model year. Preserve its
   ACT/365F origin, source roles and fit provenance. Admit the exact zero
   limits. Keep this product profile scalar-only; a schedule needs a
   separately qualified contract across payoff, tree, MC and risk.
   Reject partial, mixed, monetary or wrong-family coefficients,
   independent fitted-path initial rates and implied-volatility proxies.
   A prepared lattice is derived from the admitted block.
   [Model resolution](../valuations-framework/model-resolution.md),
   [dated Hull–White fit](../models-analytic/rate-clock-and-projection.md).

2. **Prepare exact notification and deliverable schedules.** Use
   [record 07](07-european-swaptions.md#accepted-decision-for-checked-european-swaption-pricing-and-risk)
   for European exercise and [record 03](03-interest-rate-swaps.md)
   for the IRS terms. A Bermudan has a strictly ordered authored
   or convention-resolved list of unique notice cuts, each with a
   mapped effective start and independently resolved deliverable leg
   grids. Require one common contractual maturity and consistent
   elected economics; reject non-co-terminal contracts in both engines.
   The notice is no later than either actual leg start. A stub is
   included only as an actual contract term. Do not clip accrual,
   reset or payment dates to a decision node, synthesize rights from
   coupon dates, sort/deduplicate a malformed list or infer a calendar.
   An explicitly elected inclusive lockout removes notices on or
   before that cutoff. Retain exact event phases and order for coincident
   events. Admit a time-zero decision only when the cut establishes
   that right on the selected clock; a future same-day decision needs
   a qualified intraday mapping. Earlier rights must be authoritatively
   resolved before valuing the survivor. Apply record 07's lapse, cash
   entitlement and exact-once handoff after actual exercise; model
   exercise never creates actual delivery or receipt.
   [Strata exercise/start-date terms](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swaption/SwaptionExercise.html).

3. **Value the dated underlying with the checked Gaussian kernels.**
   At each exercise node use the centered continuous affine discount
   bonds under the bound curve, clock and scalar block. Sum
   `A_e=Σ_i α_i D_e(p_i)` over eligible fixed cash and obtain the
   signed floating PV per notional `L_e` before payer/receiver side.
   Known observations use their actual eligible cash. For an unresolved
   term coupon require the admitted chronology
   `e≤f≤Sx<Ex`, actual payment `P≥Sx` and positive contractual index
   accrual. At the fixing event itself use the event's admitted phase.
   With `G_P` the checked payment-measure projection gross growth of
   the actual fixing and benchmark interval, the per-notional coupon is
   `(α_coupon/α_index)(G_P−1)+s α_coupon`. Apply its dated discount
   once; the adjustment at `P=Ex` is exactly one. Do not project a
   path-dependent pre-notice reset using only the decision state.
   For fully future contiguous reciprocal-bond overnight resets with
   fixing at each interval start, weights equal to the exact benchmark
   accruals, no shifts and payment at the last interval end, the
   same-curve discount tower gives compounded PV per notional
   `D_e(S)−D_e(E)+s α_coupon D_e(E)`. Require those exact conditions;
   another discrete schedule needs its own qualified law. Alternatively
   permit an explicitly selected continuous-accumulation approximation
   for a matched unseasoned, unshifted overnight interval, with exact
   start/end/payment dates and spread outside the accumulation. Use its
   own accepted payment adjustment and require the approximation's
   supported range. Lookback, shift, cutoff and unresolved retained
   past observations are outside this approximation. Require a
   positive representable annuity where the profile uses a swap rate.
   Propagate missing observations, invalid factors and failed arithmetic;
   no artificial zero swap or zero payoff.
   [Accepted Hull–White term and continuous payment measures](../models-analytic/hull-white-analytic-kernels.md),
   [Henrard's model implementation](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

4. **Compare immediate exercise and continuation in the same units.**
   Physical exercise at node `e` has value
   `N max(ε(L_e−K A_e),0)`, with `ε=+1` for payer and `−1` for
   receiver. This is the decision-time PV of the exact deliverable.
   Collateralized Cash Price may use it only on the matched settlement
   basis in record 07. Keep live ParYield on record 07's quote route.
   Its frozen-forward annuity does not define a Hull–White exercise
   payoff. Applying the flat-yield annuity to the node par rate can
   leave the compounding domain; support would require a separate
   payoff law qualified over the selected model's states. Do not
   floor the yield base or approximate it at a projected rate.
   Use the decision-time conditional value of the supported cash claim
   and never discount it to payment again. A later determination
   requiring extra uncertain information needs its own qualified
   conditional exercise law, or is unavailable. Do not elect a cash
   method by engine or admit the retired `IsdaParPar` and `ZeroCoupon`
   proxies. A live model's exercise mark cannot replace the authorized
   post-exercise settlement amount.

5. **Prepare and reuse one exact checked lattice.** Require explicit
   tree grid resolution, work limits and positive relative-DF replay
   tolerance under [numerical record 007](../models-numerical/hull-white-event-grid-and-affine-bonds.md).
   Use the actual immutable grid with exact mandatory nodes, including
   origin, all admitted exercises and required dated events through
   the last required payment. Include any needed settlement or
   deliverable horizon; an explicitly qualified affine tail may cover
   later cash. Drop the 100-step default, nearest-time lookup,
   tolerance-based event matching and resubdivision of a prepared grid.
   Fit the centered trinomial to checked origin-relative source
   discounts and validate replay before publishing. Exercise uses
   continuous affine underlying bonds; continuation uses the actual
   finite-lattice discount and branching. Qualify the distinction.
   Reuse a Rust-owned prepared tree only for the same origin, source
   snapshot and roles, selected block, bound controls and law, exact
   required-event membership and covered horizon; require exact grid
   identity when a grid is frozen. Keep product policy state request
   local. Shocks preserve required grid knots, rebuild the changed
   model and replay its fit. A date roll recomputes required knots from
   the original dated terms on the new origin.
   [Hull and White's trinomial construction](https://ndpearson.wordpress.com/wp-content/uploads/2015/07/hullwhite-1994.pdf).

6. **Use the pricing tree's decisions for exercise statistics.** At
   every eligible node, including a genuine origin or terminal right,
   compare the checked continuation with exercise. Exercise when
   `E>0` and `E≥C`; positive ties exercise, zero/zero continues, and
   the final eligible decision has zero remaining option continuation.
   PV and diagnostics use the same stored actions and branches. Remove
   the separate today maximum applied to price alone. Propagate
   undiscounted risk-neutral surviving mass in stable order and absorb
   first exercise once. Publish ordered date probabilities, including
   computed zero rows, total exercised and residual no-exercise mass.
   Check mass accounting with an arithmetic error bound; preserve
   small representable probability, with no pruning or renormalization.
   Report `ExpectedExerciseTime=Σ t_i p_i/Σ p_i` in model years,
   conditional on exercise, only when total exercise mass is positive.
   Genuine exercise at origin can yield zero years. A zero exercise
   probability gives an unavailable time. Keep the metric tree-only;
   these probabilities are model outputs, separate from actual exercise.

7. **Simulate rates and discounting jointly for Hull–White LSMC.** Use
   the accepted centered-state and integrated-rate Gaussian transition,
   including exact curve shift, conditional covariance root and checked
   log bank account. At origin both centered state and log bank are
   zero. Keep actual grid nodes for required events and explicit extra
   knots. Bound the grid through the last required path event; a
   qualified exercise-time PV can value later deliverable payments
   without extending the simulation. Remove fitted theta, trapezoid
   bank integration, implicit monthly or minimum-step subdivisions,
   and zero/NaN path substitutions. One
   Philox block supplies two ordered normals per positive interval;
   mirror both for each antithetic partner and preserve their slots
   even on zero-noise intervals. A decision-time underlying or cash
   claim PV from item 4 is discounted from its chosen decision time
   on the same path. Actual realized cash, where a separately
   admitted owner route supplies it, stays at its payment time.
   Use one basis per claim and never insert both values.
   [Exact joint paths and bank account](../models-analytic/hw1f-monte-carlo-discounting.md),
   [Philox stream profile](../models-monte-carlo/02-random-streams.md),
   [Hull–White simulation and payment measures](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

8. **Train a checked continuation policy on disjoint streams.** Require
   explicit counts for used training and pricing stages, seed, antithetic
   choice, execution, polynomial degree and applicable SVD controls.
   Use fixed budgets of 2 through 10,000,000 base groups per sampled
   stage, with the accepted engine limits and checked aggregate
   path/grid/fit work. Assign training base IDs `[0,N_train)` and
   pricing IDs `[N_train,N_train+N_price)`, keeping pairs together.
   Remove the 50,000/42/degree-three and in-sample defaults. Use an
   explicit intercept-only degree zero or degrees one to three in the
   sole centered factor at the decision, standardized using eligible
   training rows only. This basis assumes the admitted payoff has no
   other stochastic state. On positive immediate-value training legs
   regress the later policy's selected value, discounted back to this
   decision by the same-path bank ratio. Each eligible leg is one
   equally weighted row. If fewer than `max(2,p)` distinct training
   groups qualify for `p` columns, freeze continue-only and report
   coverage. A failed sufficiently covered fit errors; no degree
   downgrade, pathwise hindsight or hidden replacement. Use one
   checked full-rank SVD predictor and preserve its training transforms.
   [Regression contract](../models-monte-carlo/24-regression-policies.md),
   [Longstaff–Schwartz](https://galton.uchicago.edu/~mykland/346W05/Longstaff.pdf).

9. **Freeze decisions and price the trained policy out of sample.**
   Positive immediate exercise must strictly beat fitted continuation;
   equality continues. At the final eligible node exercise only if
   positive. For a qualified origin compare immediate value with mean
   training continuation (complete pair means when paired) and
   freeze that decision before pricing. Replay per-date rules forward
   on held-out streams and take first exercise. An inactive sigma
   domain uses the deterministic path, comparing checked dated
   discounted values with later-date ties and no simulation. A lone
   stochastic decision requires no regression; a frozen origin exercise
   requires no pricing paths. Report work literally. For a sampled
   price, use one checked observation per complete group with the
   accepted fixed-sample standard error and 95% Student-t interval.
   Report the trained policy's estimate. Its population value cannot
   exceed the optimal value on the same grid and law; the sampled
   estimate need not stay below it. Publish counts, seed as an exact
   integer, policy/basis identity and date-level coverage and fit
   diagnostics. The interval is conditional
   on the trained policy and describes pricing sampling alone.
   It does not measure fit, grid, model or stopping-policy error.
   Remove in-sample price reporting and PV-only today or European
   floors. Do not publish the tree's expected-time metric for LSMC.
   [Holder policy comparison](../models-monte-carlo/23-lsmc-exercise.md),
   [fixed-sample estimation](../models-monte-carlo/01-engine-and-estimation.md).

10. **Reprice named Hull–White risks on their actual axes.** Use
    signed native-currency delta and gamma per decimal rate and
    its square on the exact shared evaluator's continuous zero-rate
    axis. Shock it once across its roles, preserving cut, contractual
    terms, grid and economic coefficients, and rebuild the deterministic
    curve fit and affected payoff. Require explicit represented steps,
    comparable raw center and sides, and the checked base-local
    stencil. Apply the accepted DV01/source/bucket rules to
    supported curve measures; replaying source quotes or refitting
    economic coefficients requires a separate explicit qualified basis.
    `hw_sigma_vega` moves the selected scalar short-rate sigma with
    explicit step and reports per `.01`; hold mean reversion, curve,
    grid and fit selection. Use both full sides if supported (including
    an exact zero lower sigma). Use the full up step and matching
    center only if the lower move would cross zero. Missing, collapsed
    or failed sides do not invoke this rule. Remove implicit rate and
    sigma steps and the European half-sigma shrink. Keep option quote
    vega and projected-forward partials separate.
    [Shared direct rate risk](../valuations-framework/finite-difference-rates.md),
    [shared option risk](../valuations-framework/option-sensitivities.md).

11. **Pair full model LSMC reprices and reconcile public consumers.**
    For admitted full reprices, retrain each scenario on matched
    training draws, freeze its rule and apply it to matched pricing
    groups disjoint from training. Verify common grid, seed, stage,
    base-group, leg and interval slots; the same seed alone is
    insufficient. Difference complete comparably discounted leg PVs
    before group folding. A deterministic scenario contributes its
    checked constant to each matched observation without fictitious
    draws. Report paired sampling intervals when applicable and keep
    training, finite-step and model error separate. Keep any deliberately
    frozen-policy risk separately identified. Preserve active sources,
    calculation axes and work diagnostics across existing native and
    registry routes, report consumers, Python/WASM typed and tagged
    envelopes, schemas and parity ledger. Keep prepared lattices and
    learned policy objects Rust-owned. Qualify the actual payoff and
    lifecycle, finite tree versus independent continuous benchmarks,
    exercise mass, exact joint discount, training independence and risk
    convergence separately. Same-engine roundtrips, local comparator
    tests and nominal sampling intervals cannot qualify all routes.
    [Paired estimator rules](../models-monte-carlo/25-finite-difference-greeks.md),
    [metric orchestration](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice, scope and rationale

The default Bermudan model is a calibrated one-factor Hull–White trinomial tree. European HullWhite1F uses the same cashflow exercise kernel at one date; Bermudan MonteCarloHullWhite1F is an explicit LSMC alternative. Both are **single-curve** implementations: projection and discount IDs must agree. The detailed calibrated tree geometry/bond formulas are owned by [models-numerical](../models-numerical/hull-white-event-grid-and-affine-bonds.md); rate stepping and affine model kernels are owned by [models-analytic](../models-analytic/hull-white-analytic-kernels.md). This record states product cashflows, exercise and regression policy, not approval of their approximation error.

### Exercise cashflows and conventions

1. Resolve fixed/float leg sentinels using IRS helpers. Accept simple float coupons or compounded coupons with exactly zero lookback, observation shift or cutoff; reject other observation modes. Keep periods whose accrual end and payment both follow exercise. Clip each remaining accrual start and float reset to exercise, recomputing accrual on that truncated interval.
2. At model time t calculate `A=sum(alpha_i*P(t,pay_i))`. Return zero swap rate, annuity and payoff if A is nonpositive. Float PV per notional is `sum((P(t,start)/P(t,end)*delayed_coupon_adjustment - 1 + spread*alpha)*P(t,pay))`. The shared Hull–White adjustment distinguishes simple versus compounded delayed coupons. `S=FloatPV/A`.
3. Physical/CollateralizedCashPrice/IsdaParPar exercise is `N*A*max(sign*(S-K),0)`. ParYield instead uses regular-frequency par annuity from the remaining period count, times a bond to max(original start, exercise); ZeroCoupon uses summed remaining fixed accrual times a bond to the cashflow horizon. European supports these choices; Bermudan always uses CollateralizedCashPrice for cash settlement.
4. Model times are ACT/365F from valuation. Bermudan exercise dates must be strictly increasing in `[underlying_start,underlying_end)`, and lockout removes dates **on or before** lockout. The convenience schedule uses no stubs, weekends-only ModifiedFollowing payments, with original first exercise retained. Future exercise lists exclude today; the public pricer subsequently takes max(continuation, deterministic exercise today) when today is effective. No surviving exercise or matured underlying returns zero before that today adjustment.

### Tree algorithm and prepared-model controls

Tree steps default 100; an instrument tree-step override wins. Resolve a complete scalar HW parameter pair through the [shared swaption-family resolver](../valuations-framework/model-resolution.md), which orders instrument overrides, a supplied prepared-model pair and prefitted market scalars; sigma schedules are rejected by that resolver. Build the tree to the last contractual payment with exercise times mandatory. Start terminal option values at zero except a terminal exercise node, then take `max(discounted continuation, exercise)` at each exercise step. European uses one mandatory expiry and its terminal intrinsic shortcut; past expiry has zero value. The tree valuator rejects NonCoTerminal.

A prepared tree stores valuation date and a serialized discount-curve snapshot. Reuse rejects changed date/curve, insufficient horizon, different resolved parameters, or explicit step-count mismatch; missing mandatory exercise nodes fail time lookup. Metric setup freezes resolved parameters into the instrument. Bumped repricing discards the cached tree, recalibrates to the bumped curve, and retains the cached grid as mandatory times, shifted for date rolls. Thus cached-tree reuse is not silent reuse of an unbumped calibration.

Exercise probabilities first reconstruct the tree continuation policy, then propagate undiscounted surviving transition mass forward. A node exercises on `exercise >= continuation` with positive exercise, removes that mass and records probability. Mass below `1e-15` is skipped. ExpectedExerciseTime is `sum(t*p)/sum(p)`, conditional on exercise; it returns zero below total probability `1e-12`. The metric is available only under tree policy. The separate exercise-today PV adjustment is not incorporated into this future-date probability calculation.

### Hull–White LSMC algorithm and controls

1. Defaults are 50,000 independent base streams, seed42, antithetic=true, minimum2 steps/event gap, polynomial degree3, in-sample regression. An instrument mc_paths override changes base streams; seed/basis/OOS remain pricer config. Shared event grid also enforces its monthly time-density floor.
2. Rebase the discount curve to valuation, calibrate theta on the event grid and estimate the initial instantaneous forward. Draw Philox substreams per base stream; antithetic partners negate every normal. ExactHullWhite1F steps short rates, while money-market discount factors use **trapezoidal** rate integrals. Conditional bonds use `exp(lnA-B*r)` and require finite positive output.
3. At final exercise retain positive exercise payoff. Work backward. On ITM training paths, regress the future chosen payoff multiplied by `B(now)/B(future)` against the supplied basis evaluated on swap rate and auxiliary annuity. The product adapter supplies PolynomialBasis `{1,S,...,S^degree}`, which **ignores the auxiliary annuity**. Fit only when ITM training count is greater than basis count plus10; otherwise skip exercise at that date. Shared least-squares failures propagate.
4. Apply the fitted rule to every path: exercise only if immediate value is positive and exceeds fitted continuation. Divide final chosen payoff by its pathwise bank account. Nonpositive bank factors cause a zero contribution in the local fallback. Pair antithetics before computing errors; optional split mode trains on even base streams and prices odd ones. Shared estimate requires at least two pricing estimators. Report standard error, 95% interval, pricing count and seed.

### Risk and alternatives

Bermudan Delta bumps the discount curve by configured ±bp and divides by twice the decimal bump. Gamma uses ±10bp and the second difference divided by squared decimal bump. HwSigmaVega bumps resolved sigma by the shared HW increment, central when sigma exceeds the bump and forward otherwise, scaled to absolute0.01 sigma. It is model-parameter risk, not swaption-surface vega. Same seeds give common random numbers for MC repricing. ExpectedExerciseTime errors for LSMC.

Tree refinement versus LSMC grid/regression/sample refinement are distinct accuracy controls. Out-of-sample LSMC is supported but off by default; exact integrated-rate sampling is not used. Standard errors exclude model, regression and discretization bias. Low-ITM skipped exercise is a material policy. Although the tree explicitly rejects NonCoTerminal, the HW LSMC path does not inspect this enum and values one common terminal swap maturity. The expected-time metric excludes the separate exercise-today value. No independent accuracy qualification for clipped mid-coupon exercise is established.

## Evidence

- [Bermudan orchestration and cache](../../../../finstack-quant/valuations/rates/src/swaption/bermudan.rs), [European HW adapter](../../../../finstack-quant/valuations/rates/src/swaption/hw_pricer.rs), [contract validation](../../../../finstack-quant/valuations/rates/src/swaption/types/bermudan.rs), [exercise schedule](../../../../finstack-quant/valuations/rates/src/swaption/types/definitions.rs).
- [Cashflow exercise](../../../../finstack-quant/valuations/rates/src/swaption/pricing/hw_cashflows.rs), [tree policy/probabilities](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tree_valuator.rs), [LSMC kernel](../../../../finstack-quant/valuations/rates/src/swaption/pricing/monte_carlo_lsmc.rs), [bond helper](../../../../finstack-quant/valuations/rates/src/swaption/pricing/swap_rate_utils.rs), [risk](../../../../finstack-quant/valuations/rates/src/swaption/metrics/bermudan_greeks.rs).
- [Bermudan tests](../../../../finstack-quant/valuations/rates/tests/swaption/bermudan_pricing.rs) locate tree/MC comparison, monotonicity, same-day exercise and parameter rejection evidence. The grid/seed acceptance case is slow/ignored in ordinary runs. [Numeraire tests](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tests/numeraire.rs) implement pathwise reference estimators; they share model primitives and do not constitute independent market validation. No tests executed.
- [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo), [Longstaff–Schwartz](../../../REFERENCES.md#longstaff-schwartz-holder-stopping-and-independent-policy-pricing) and [shared regression policies](../models-monte-carlo/24-regression-policies.md) provide provenance and context. The European HW source tests include a comparison based on clipped coupons; that test is evidence of the old payoff law.

### Independent review evidence

- A Ho–Lee exercise-node term fixture with `κ=0,σ=.02,D(t)=exp(−.04t)`,
  fixing `.25`, benchmark `.3→.8`, payment `.95`, coupon/index
  accrual `.48/.5`, decimal spread `.001`, strike `.035` and
  notional one million gives log payment adjustment `−.0000075`,
  floating PV per notional `.01912519848713` and payer exercise
  value `2,951.62108016`. Moving the fixing to the decision at zero
  would increase value by about `7.07153291`.
- On a one-step fixture with unit discount and branch probabilities
  `1/6,2/3,1/6`, only one outer child can exercise for `6` at
  time one. Origin continuation equals `1`. An eligible origin
  payoff of `1` exercises immediately with probability one and
  conditional expected time zero. Origin payoff `.75` preserves
  the same PV but moves exercise to time one with probability `1/6`
  and no exercise with probability `5/6`. Twenty outer `1/6`
  branches produce positive mass of about `2.7351e−16`.
- For two years of Ho–Lee with flat forward `.03` and sigma `.02`,
  integral noise variance is `.001066666667`, state/integral
  covariance `.0008` and the shift integral `.060533333333`.
  Exact expected discount is `.941764533584`; the single endpoint
  trapezoid has expected discount `.941387903102` despite exact
  endpoint evolution.
- For a one-coupon payer with notional one million, exercise/start
  at year one and end/payment at year two, unit accrual, strike
  `.04`, flat `.03` and Ho–Lee sigma `.01`, the exercise value is
  `N[1−1.04 D_e(2)]_+`. A continuous bond-put reference gives
  `940.14762251`; the corresponding receiver is `9,929.72900162`.
  These are convergence targets for the numerical routes.
- For an intercept-only fit, later chosen node values `6` and `2`
  with same-path factors `.98` and `.95` give mean training
  continuation `3.89`. A fitted exercise value of `4` stops,
  equality continues. Origin exercise at `100` against training
  continuation group PVs `98,100` is fixed before holdout; pricing
  groups cannot revisit the decision.
- Contiguous overnight reset growth and discount can be checked in
  a two-step finite state example. For a first-step unit bond `.99`
  and second-step node bonds `.98` and `.97`, the conditional
  expectation of the second-step discount times reciprocal bond is
  one in each state. Folding the first-step reciprocal bond leaves
  one for the discounted gross accumulation at its start. From a
  decision before start this gives `D_e(S)`, before subtracting
  `D_e(E)` and adding spread.
- A one-period zero-sigma payer with notional one million, exercise
  and start at year one, end and payment at year two, unit accrual,
  strike `.04` and flat continuous rate `.05` has positive-branch
  PV `1m[exp(−.05)−1.04 exp(−.10)] = 10,198.50974332`. A matched
  ±1 bp direct stencil gives delta approximately `930,832.415975`
  and gamma `−2,812,894.250928` in decimal-rate units. Cent rounding
  the center and sides before differencing makes gamma `−3m`.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
