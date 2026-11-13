# MODELS-AND-VALUATIONS-CREDIT-05: Leverage-driven hazard and issuer PIK exercise

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Current behavior observed in the source below. The accepted contract requires Rust and caller reconciliation.
- Validation status: Independent scalar formula and boundary examples evaluated. Production, host, market and issuer-policy qualification were not run.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked leverage hazard and PIK election models

1. **Keep three explicit leverage-to-intensity maps.** Use a finite
   nonnegative debt-to-assets ratio `L` with no upper bound other than
   representability. Parametric maps require finite `h0>=0`, finite
   `L0>0` and finite nonnegative exponent `a` or sensitivity `s`. Hazard
   is an annual decimal intensity on the caller's model clock. Use
   `h0*(L/L0)^a` for power law and `h0*exp(s*(L−L0))` for exponential.
   After validation, `h0=0` gives exact zero, the anchor gives exact `h0`,
   and a zero coefficient gives a constant including at zero leverage.
   The power law at zero leverage with positive exponent gives zero.
   Use stable log-ratios near and away from the anchor and compute the
   complete scaled product. Remove negative-input floors and the `1e6`
   hazard cap. Fail an unresolved result, including a required positive
   intensity lost to zero. These parametric coefficients are supplied
   model assumptions. [Reduced-form intensity background](https://web.stanford.edu/~duffie/jstorlinks/ds2.pdf).

2. **Preserve authored table knots and post-PIK leverage.** Require equal,
   nonempty leverage and hazard axes, finite nonnegative values, and
   strictly increasing leverage knots in supplied order. A single point
   gives a constant. With more points use checked linear interpolation,
   exact knot values and flat endpoints. Preserve a nonmonotone authored
   hazard axis. A table has no separate reference pair. Post-PIK lookup
   uses the finite nonnegative already-accreted balance over finite
   positive same-basis assets. Check the quotient before evaluating the
   map. An instrument balance divided by firm assets is explicitly an
   instrument leverage proxy when it represents only part of issuer debt.
   The coupon owner determines capitalization. Data, clock, calibration
   and the intended leverage basis belong to the caller.
   [Checked interpolation](../core-numerics/value-interpolation.md),
   [parent-coupon cash and PIK amounts](../cashflows/05-coupon-cash-and-pik.md).

3. **Anchor market hazard locally on its frozen source clock.** For the
   actual dated window map both endpoints on the same retained source
   base, day count and context. Require finite source coordinates
   `0<=a<b` and use checked `J(a,b)/(b−a)`, where `J` is the canonical
   integrated hazard over the window. Exact zero integral gives zero.
   Retire the endpoint-survival helper after moving the revolver to the
   local integral. Remove absolute-survival floors and default-context
   remapping from this calculation. The revolver owns choice of tenor,
   actual model clock, initial spread and attainable CIR mean. The credit
   triangle `s≈(1−R)h` is a caller-selected first-order level approximation
   with explicit recovery and assumptions. It carries no quote-reprice or
   issuer/index transfer claim. [Canonical hazard source](../core-market/hazard-survival.md),
   [revolver valuation](../valuations-other-fixed-income/11-revolver-monte-carlo-credit.md).

4. **Retain the checked local CIR diffusion conversion.** Require finite
   nonnegative fractional annualized spread volatility `ν` and finite
   nonnegative annual decimal reference spread `s_ref`. Use
   `σ_CIR=ν*sqrt(s_ref)` for the spread process diffusion
   `σ_CIR sqrt(s) dW`. It matches local absolute volatility `ν*s_ref`
   at the supplied reference. At zero reference spread select a zero
   coefficient, since that local equality alone does not identify it.
   Zero fractional volatility also gives exact zero. Check range and
   reject a nonzero coefficient lost to zero. This conversion stays
   Rust-only and does not set mean reversion, enforce Feller or infer a
   volatility quote. Apply the accepted CIR process rules in the consuming
   product. The term-loan lattice consumes its own absolute hazard
   volatility. [CIR square-root diffusion](https://pages.stern.nyu.edu/~dbackus/BCZ/discrete_time/CIR_Econometrica_85.pdf),
   [credit-line joint process](../models-monte-carlo/11-credit-line.md).

5. **Require only the selected rule's inputs.** Select threshold, logistic
   or nested equity comparison explicitly. For a feature rule require
   its selected annual hazard, debt-to-assets leverage, or signed
   distance to default. Hazard and leverage must be finite and
   nonnegative. Distance must be finite when supplied and carry its
   caller-owned definition, measure and horizon. A PD-equivalent
   annualized intensity from a terminal model has a different basis
   from a mapped instantaneous hazard. Require the caller to select
   and identify the feature basis. A missing required feature is an
   error. Use checked immutable state/configuration and retire mutable
   or defaulted decision inputs, missing-distance-as-zero, inferred
   assets, negative amount clamping and invented randomness.
   [Structural default and distance](04-structural-default.md).

6. **Use strict thresholds and stable logistic decisions.** A threshold
   is finite in the selected feature's units. Above and Below use strict
   `>` and `<`; equality chooses cash. Threshold uses no uniform.
   Logistic requires finite intercept and signed sensitivity, a checked
   fused predictor `η=intercept+sensitivity*x`, and an explicit finite
   uniform `u` in `[0,1)`. Elect PIK exactly when `u<logistic(η)` in the
   model. For `u>0` compare `log(u)−log1p(−u)<η` without materializing
   a rounded probability. At `u=0` any finite predictor elects PIK.
   The rule gives a conditional scenario choice under supplied
   coefficients. Fit, provenance and empirical policy evidence stay
   with the caller.

7. **Compare two explicitly priced actions at one eligible coupon.** Rename
   the current optimal-exercise variant to `EquityComparison`. Require
   finite positive firm asset value `V` and modeled debt balance `N`,
   plus separately supplied nonnegative cash outflow `C` and PIK principal
   addition `P`, all on one monetary basis. The actions start at
   `(assets, debt)=(V−C,N)` and `(V,N+P)`. Each debt value is that action's
   flat modeled barrier and terminal strike. Product input may select
   equal amounts explicitly. Check arithmetic and assess both origin
   cushions inclusively and independently from the supplied amounts.
   A breached action gives zero modeled equity. Preserve the mathematical
   equality of the two origin tests when `C=P`, and fail if required
   balances or comparisons cannot be represented reliably. Both actions
   must be contractually eligible before invoking the comparison. Actual
   issuer elections, contract rates, notices and amount allocation belong
   to the instrument. [Example filed PIK-toggle indenture](https://www.sec.gov/Archives/edgar/data/1550043/000119312512238523/d350029dex101.htm).

8. **Choose explicit discrete monitoring and paired replay.** Require
   finite positive horizon, positive step and path counts, finite
   continuous `r`, nonnegative finite `σ`, explicit `u64` seed and stream
   and a positive work budget. Check that `paths*steps` fits the budget
   and validate resources before execution. Use the canonical uniform grid with its exact horizon and
   actual adjacent step widths. This hypothetical GBM has zero ongoing
   payout. Its common log growth increment is
   `(r−σ²/2)Δt+σ sqrt(Δt) Z`. Use one checked CORE-NUMERICS-17 PCG
   stream per decision. Generate one common normal per path-step in
   fixed path-major order, independent of which leg is alive. Remove
   inferred monthly monitoring and uniform-bit seed derivation. Zero
   volatility uses one deterministic pair and no random draws. When
   both actions are breached at origin or both action amounts are zero,
   the known tie needs no draws. Use checked log growth and stable
   log-ratio comparisons. Touch or breach at origin or any actual knot
   extinguishes that action's equity, including at maturity.
   There is no between-knot correction. Reproducibility follows the
   selected replay profile and target arithmetic.
   [Core random draws](../core-numerics/pseudorandom-and-bridge.md),
   [core time grids](../core-numerics/time-grids.md),
   [discrete barrier monitoring](https://www.columbia.edu/~mnb2/broadie/Assets/bgk_mf.pdf).

9. **Decide from the checked paired equity difference.** A surviving action
   pays `max(V_T−B,0)` in this hypothetical model, while a breached one
   pays zero. When both survive, compute the paired difference as
   `C*exp(Y)−P` for common terminal log growth `Y`, using
   `(C−P)+C*expm1(Y)` near zero and checked scaled arithmetic. This
   preserves small differences without subtracting two large separately
   rounded equity sums. Stream the checked paired mean using scaled
   compensated arithmetic. A common positive discount factor cancels
   in the comparison. Elect PIK only when the mean difference is strictly
   positive. An exact tie, including two zero equity legs, chooses cash
   under the model. Return the action, mean undiscounted difference,
   origin-breach flags and work/replay controls for the equity comparison.
   Unresolved arithmetic or work exhaustion fails the whole decision.
   This compares the immediate actions on one grid. It supplies no
   multi-coupon exercise optimization, variance-reduction guarantee or
   evidence that an issuer follows the rule.
   [Checked lognormal restoration](../models-monte-carlo/04-lognormal-diffusions.md),
   [checked accumulators](../core-numerics/sample-statistics.md).

10. **Restore checked state and reconcile the actual consumers.** Canonical
    wire state carries the selected variant and its semantic fields.
    Reject unknown fields, invalid levels, missing required state and
    mismatched controls. Tabular wire omits the derived first-knot
    reference fields. Rust owns validation, maps, randomness and decisions.
    Reconcile existing Python/WASM bindings, schemas, declarations, parity
    and instrument pricing overrides, including exact seed and stream
    transfer. Propagate failures through native and WASM pricing without
    publishing a partial result. The bond must supply the selected feature
    and required randomness. A requested toggle without a model or
    required input fails. Endogenous hazard is a toggle feature in the
    current bond route. Outer default still uses the bond's structural
    barrier, and its cash outflow, notional updates, coupon timing and
    actual PIK elections remain product decisions. The revolver owns its
    market reference and feasible CIR fit. The statements owner retains
    reporting-period elections and carrying state.
    [Bond simulation](../valuations-bonds/08-merton-simulation.md),
    [bond calibration](../valuations-bonds/09-merton-calibration.md),
    [statements PIK and carry](../statements-capital/08-pik-and-carry-state.md).

## Observed behavior

- Leverage maps cap all outputs at `1e6`; the power map floors negative
  leverage, and a NaN can escape. The post-PIK division is unchecked.
  Tabular state exposes its first knot as a reference pair. The revolver
  floors absolute survival and reconstructs the source clock using default
  context. CIR conversion validates its inputs and returns an unchecked
  product. The revolver applies separate spread, recovery and CIR floors.
- Toggle state accepts finite negative levels and supplies a default coupon.
  Distance defaults to zero and missing assets are inferred from leverage
  or twice debt. Logistic compares a materialized sigmoid with an
  unchecked uniform. The equity variant clamps a negative coupon, uses a
  single amount for both actions and chooses PIK immediately on a cash
  breach, even when both actions breach.
- The nested simulation uses `ceil(12T)` steps and independent per-path
  PCG streams with common seeds across actions. It derives the seed by
  hashing the outer uniform bits, restores asset values by unscaled
  multiplication and compares two payoff sums. The bond call can use
  cash for a missing model, `0.5` for missing randomness or zero for
  an unavailable distance. Its mapped hazard affects the choice feature,
  while the outer default barrier is separate.

## Evidence

Owners: [endogenous_hazard.rs](../../../../finstack-quant/models/src/credit/endogenous_hazard.rs), [market_anchored.rs](../../../../finstack-quant/models/src/credit/market_anchored.rs), [toggle_exercise.rs](../../../../finstack-quant/models/src/credit/toggle_exercise.rs), [bond toggle caller](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/engine.rs), [revolver anchor](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/pricing/path_generator.rs). Inline tests cover selected branches, monotonicity and determinism. The current cash-breach example exercises a both-breached origin and therefore does not establish a surviving PIK alternative.

### Independent review evidence

- With assets 100 and debt increasing from 50 to 60, a power-law map
  anchored at `h0=.03,L0=.5,a=2` gives `.0432` after accretion.
  `1e−300*exp(800)` has a finite result near `2.72637e47` despite
  overflowing the isolated exponential. At `h=1e8,Δt=1e−8`, default
  probability is `0.63212056`; the hidden `1e6` cap changes it to
  `0.00995017`.
- With prior integrated hazard 800 and a future half-year intensity of
  `.04`, the absolute survival endpoints underflow, while local
  `J=.02` recovers `.04`. At `ν=.35,s_ref=.018`, the CIR coefficient is
  `0.04695743` and local absolute volatility is `.0063`.
- At finite logistic predictor `−1000`, a materialized sigmoid can round
  to zero. The `u=0` decision still elects PIK. At `V=104,N=100,C=P=5`,
  both origin tests breach and the paired difference is zero. With
  `C=5,P=3`, only the PIK action survives the origin test. At
  `V=1e16,N=1e16−2,C=P=1`, independent rounded balance calculations
  can falsely make the two origin predicates disagree.
- For deterministic surviving legs with `V=200,N=100,C=P=5,T=1`,
  the difference is `5*expm1(r)`. It is about `−.475813`, `0` or `.525855`
  for `r=−.1,0,.1`. At `V=120,N=100,C=P=5,σ=.3,r=0,T=1`, a single
  terminal observation gives Black–Scholes equity values `21.67119061`
  and `22.20907049` for cash and PIK. Under continuous flat-barrier
  monitoring at zero drift the stopped asset martingale gives `15`
  for both. The grid choice can therefore change the comparison.
- Source tests were located and independent arithmetic examples evaluated.
  Qualification still requires range and grid cases, replay and paired
  statistical evidence, schema/host and full product routes, and
  applicability to supplied market data and issuer decisions.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
