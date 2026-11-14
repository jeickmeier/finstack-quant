# VALUATIONS-RATES-09-LMM-BERMUDANS: Bermudan LMM displacement, terminal numeraire and regression fallback

- ID: VALUATIONS-RATES-09-LMM-BERMUDANS
- Decision status: **Accepted — complete checked LMM Bermudan pricing and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, numerical, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked LMM Bermudan pricing and risk

1. **Admit one exact physical co-terminal contract.** Explicitly select
   `LmmMonteCarlo` for a single-currency payer or receiver with positive
   exact Money notional and an exact signed strike. Bind one actual
   typed shared evaluator for projection and discount, including both
   leg roles. Equal IDs do not establish sharing. Resolve both legs
   under [record 03](03-interest-rate-swaps.md) and exact notices,
   effective starts, lockout and contractual lifecycle under
   [records 07](07-european-swaptions.md) and [08](08-hull-white-swaptions.md).
   Require matching actual contiguous fixed and simple term floating
   periods, exact benchmark intervals and matching positive contractual
   coupon/index accruals. Admit a contractual stub only when it still
   satisfies those checks. Require zero spread and fixing and payment
   lags, actual reset at the period start, and actual payment at the
   accrual end. Each ordered eligible notice and deliverable start maps
   to the same exact tenor start `T_k<T_n`; the actual final payment
   fixes `T_n`. At future exercise the just-fixed rate is admitted
   only when the modeled fixing precedes the decision on the declared
   cut. Reject incompatible same-date chronology or ambiguous
   date-only state. Reject cash, non-co-terminal, off-tenor, mismatched
   schedule or multicurve requests, including before any origin or
   no-future-date branch. Do not clip a coupon or substitute an HW
   payoff. At or after actual exercise require authoritative state and
   hand off the deliverable once. A proved lapse, extinguishment or
   handoff can skip unused stochastic inputs.

2. **Keep the retained forward strip and origin exercise distinct.**
   Use ACT/365F from the effective valuation origin for model knots and
   the actual contractual fractions `τ_i` for cash. For the future
   stochastic strip retain the suffix starting at the first strictly
   future eligible exercise, together with each period's original
   index and the full contractual period count. Earlier forwards do
   not affect terminal-measure drift or payoffs of this suffix. Build
   each retained initial forward as
   `expm1(log D(V,T_i)−log D(V,T_(i+1)))/τ_i` using qualified
   dated relative factors on the evaluator's native clock; keep
   `D(V,T_n)` from that same evaluator. An eligible origin right
   remains in the policy. Value it directly on the exact matched IRS
   cash basis at the cut, using any eligible actual fixing even when
   it differs from the curve projection, and only a qualified
   projection when the fixing is still unpublished. Missing required
   actual history errors. Put this decision value in terminal units
   for comparison without overwriting the fixing, changing the curve
   or inserting that rate into the future model strip. A genuine
   origin-only right is checked directly, without a stochastic request
   or a post-estimate maximum.
   [Accepted LMM state and measure](../models-monte-carlo/10-lmm.md).

3. **Choose one explicit displacement and loading profile.** Admit a
   finite signed common displacement `d`, an explicitly selected
   versioned constant two-factor profile and a finite positive
   `lmm_base_vol` scale `a` per square root model year for a live
   stochastic strip. Admit the existing linear shape as the explicit
   `linear_two_factor_v1` profile: for zero-based original index `i` and
   original count `m`, `g_i=(1−.4 i/m,.4 i/m)`,
   `λ_i=a g_i`. Retain both and only those two lanes. Its covariance
   rate is `λ_i·λ_j`; this prescribes a shape without estimating
   factors, correlation or a volatility term structure. Remove the
   curve-derived 1% shift and the implicit shape selection. Under the
   [accepted shifted support](../models-monte-carlo/10-lmm.md#accepted-decision-for-checked-terminal-measure-lmm-paths)
   require `F_i+d>0`, `1+τ_i F_i>0`, and exact `τ_i d≤1`
   for the whole retained strip. Do not repair or floor failures.
   A literal zero shift is allowed if the support passes. Consume
   authored inputs or a qualified existing fit bound to this exact
   profile, shift, source, basis and origin. No automatic quote
   surface, fit or conversion from Black or Normal swaption volatility.
   Reconcile the explicit upstream scalar calibration helper with
   the same contractual indices, shape and displacement; it owns the
   target and fit qualification under
   [calibration record 09](../calibration-rate-models/09-lmm-rebonato-calibration.md).
   Other loading tensors or fitted covariance profiles require their
   own product qualification. [Denson and Joshi on displaced LMM](https://fbe.unimelb.edu.au/__data/assets/pdf_file/0011/2591831/201.pdf).

4. **Run the accepted predictor-corrector on exact dated knots.** Resolve
   a checked owner grid from origin through the last required future
   exercise, including every retained reset and any applicable
   coefficient knots through its horizon and any explicitly chosen
   extra resolution. Preserve exact lookup and chronological event
   order. The simulation may end before `T_n`; the tenor model,
   deliverable and terminal factor must still reach it. Use the shared
   displaced log predictor-corrector and factor draw layout under the
   terminal measure. A rate fixing at the end of a step participates on
   that incoming step and is observed and frozen at the node; later
   moves preserve the fixing. Draw two ordered checked independent
   normals on each actual interval, reusing them in both stages and
   reflecting both for a paired leg. Remove snapping and epsilon
   alive-set tests, implicit monthly and minimum-step subdivisions, and
   skipped or dropped path states. Finite-step drift for interacting
   forwards can bias bond moments and option values; increasing
   sample count does not remove that bias.
   [Exact time grids](../core-numerics/time-grids.md),
   [accepted LMM transition](../models-monte-carlo/10-lmm.md) and
   [Glasserman and Zhao on discrete bond martingales](https://business.columbia.edu/faculty/research/arbitrage-free-discretization-lognormal-forward-libor-and-swap-rate-models).

5. **Measure future exercise and continuation in terminal units.** At
   the exact exercise index `k`, include its just-fixed `F_k`. For
   `G_j=P(T_k,T_j)/P(T_k,T_n)`, set `G_n=1` and use checked
   tenor-bond ratios from the same admitted forwards. The payer
   unit-notional value is
   `G_k−1−K A^N`, where `A^N=Σ_(i=k..n−1) τ_i G_(i+1)`.
   Evaluate the equivalent signed cash sum
   `Σ_(i=k..n−1) τ_i(F_i−K)G_(i+1)` in scale to preserve
   cancellation and near-ATM cash. Select its positive payer or
   receiver part, scaled by notional. The same node's local annuity
   and par rate, when needed as regression features, are
   `A=A^N/G_k` and `S=(G_k−1)/A^N` with checked positive
   annuity and stable signed numerator. Keep all continuation
   responses in terminal units without discounting between exercise
   dates. Multiply the final selected terminal-unit payoff by
   `D(V,T_n)` exactly once for time-zero PV. Preserve the cash
   basis across legs, statistics and risk. Remove annuity floors,
   zero-on-invalid-numeraire and anchoring to an earlier tenor.
   [Terminal-measure derivation](../../../REFERENCES.md#displaced-lmm-and-terminal-bond-pricing).

6. **Require explicit LMM run controls and disjoint samples.** Use
   the LMM grid and exact integer effective seed, execution and
   antithetic choices, separate training and pricing group counts,
   degree and applicable solver controls. Require 2 through
   10,000,000 base groups per used sampled stage and check actual
   grid, draw, trajectory, fit and allocation limits before work.
   Fix Philox training IDs `[0,N_train)` and pricing IDs
   `[N_train,N_train+N_price)`, with both legs of a pair in one
   group and stage. Bind each draw to its actual group, leg, factor
   and step. Replace the shared Hull–White config, inherited
   path-count override and embedded path/seed/basis defaults with
   this one checked LMM configuration. No in-sample price election.
   [Fixed-budget engine](../models-monte-carlo/01-engine-and-estimation.md),
   [Philox profile](../models-monte-carlo/02-random-streams.md).

7. **Train a checked stopping rule without path hindsight.** Train
   backwards on the training legs with positive immediate exercise.
   Use explicit degree zero (intercept only) or degrees one through
   three on the ordered decision-state features `(S,A)`. Use
   training-only standardization and the checked full total-degree
   basis (column counts `1,3,6,10`). Use the checked mean for an
   intercept-only fit and full-rank SVD otherwise under
   [regression record 24](../models-monte-carlo/24-regression-policies.md).
   The two features summarize the curve for this fit; they do not
   identify its full state. Use each eligible leg's later selected
   terminal-unit value as its training response, with equal row
   weight. For fewer than `max(2,p)` distinct eligible training
   groups for `p` columns, freeze continue-only and record the
   coverage. Rank, range, shape or SVD failure on a sufficiently
   covered fit errors. Select the basis before simulation. Do not drop
   a feature, downgrade the basis, clip a prediction or compare with
   each path's realized future when exercising.
   [Longstaff–Schwartz](https://galton.uchicago.edu/~mykland/346W05/Longstaff.pdf).

8. **Freeze the policy before valuing it.** Stop only on positive
   immediate exercise strictly exceeding the learned continuation;
   equality continues. At the final right exercise only if positive.
   For a genuine origin right with future optionality, compare the
   origin payoff to mean training continuation in terminal units
   (using complete pair means when paired). Freeze that action
   before seeing any pricing path. Replay the learned rules forward
   on held-out groups and take the first stop. A lone stochastic
   right needs no regression; an origin exercise fixed by the
   trained policy needs no pricing draws. Resolve any proved
   deterministic claim directly with actual work recorded. Do not
   use an origin, European or perfect-foresight price floor. The
   population value of the frozen admissible policy is at most the
   optimal value on the same model and grid; sampling need not preserve
   that inequality for a reported estimate. Publish one observation
   per complete pricing group and a fixed-sample 95% Student-t
   interval, conditional on the fitted policy. Retain actual counts,
   exact seed, grid, origin action and per-date coverage and fit
   diagnostics. Distinguish zero sample variance from no sample.
   The interval excludes fit, grid, model and policy error. Do not
   publish the tree's `ExpectedExerciseTime` on the LMM route;
   any held-out stopping statistics must state their terminal
   measure and fitted-policy basis.
   [Shared holder policy rules](../models-monte-carlo/23-lsmc-exercise.md).

9. **Use the selected LMM pricer for direct curve risk.** Define
   delta and gamma on the actual shared evaluator's continuous
   zero-rate axis, in native currency per decimal rate and its
   square, with explicit represented widths and matched base and
   both sides. Use the base-local slope and curvature on the actual
   nodes under framework 09. Shock the evaluator once through its roles
   and rebuild projected forwards, terminal discount and responsive contractual
   valuation, and hold actual fixings, displacement, loading shape,
   scale, terms and dated knots fixed. A failed support check
   fails that shock. Separate direct rate risk from any explicitly
   authorized quote replay that recalibrates the fitted inputs.
   Use the accepted signed per-bp experiment for any curve PV01
   this route publishes. Keep quote vega, HW sigma vega and an
   unqualified LMM scale sensitivity unavailable in this package;
   any future scale or loading Greek requires its own named axis
   and units. [Shared rate differences](../valuations-framework/finite-difference-rates.md).

10. **Pair the full risk reprices and reconcile public routes.**
    Retrain each scenario's policy with matched training group,
    leg, factor and interval slots on the same grid and controls.
    Exercise timing and fitted rules can change between scenarios.
    Freeze those policies and use matched held-out pricing groups
    separate from training. Difference comparable complete leg
    PVs using each scenario's terminal conversion before averaging
    antithetic pairs and forming a paired interval. Use genuine
    raw reprices or an explicitly admitted checked Money difference;
    a finalized PV cast to binary64 is not raw. A deterministic
    scenario can supply its exact constant without claiming draws.
    Keep any later frozen-policy sensitivity on a separate basis.
    Preserve exact seeds, counts, policy diagnostics, active sources,
    fit, steps and typed errors across Rust, registry/report consumers,
    Python and WASM, generated schemas, declarations and parity.
    Keep prepared state and learned policy Rust-owned. Reconcile
    the existing calibration helper against the actual selected
    pricing profile. Qualify terminal-forward reference prices,
    nonterminal deflated bond moments, multi-forward option prices,
    support under shocks, sparse fits and policy/grid convergence
    independently. Same-kernel roundtrips do not qualify market
    calibration or full product performance.
    [Paired repricing](../models-monte-carlo/25-finite-difference-greeks.md),
    [metric ownership](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice, scope and rationale

LmmMonteCarlo is an explicit Bermudan alternative to Hull–White. It builds a displaced two-factor forward model from the single discount curve and an explicitly supplied positive `lmm_base_vol`; it does not calibrate that scale from the swaption vol surface. The low-level process/predictor-corrector algorithm is documented in [models-monte-carlo](../models-monte-carlo/10-lmm.md). The product adapter's factor shape, displacement, contract domain and stopping rule are additional choices.

### Contract gate and parameter construction

1. Require physical CoTerminal settlement, identical fixed/float discount and forward IDs, zero float spread/reset lag and simple compounding. Fixed and float start/end/frequency/day-count/stub/EOM/BDC/calendar must match; payment lags must be zero. Every actual payment must equal accrual end without business-day displacement. Effective exercises must equal accrual starts. Other contracts error.
2. Drop forward periods before the next strictly future exercise; retain original period index for loading shape. Tenor times use ACT/365F from valuation; accrual factors use actual contractual schedule alpha. Bootstrap `L_i=(D(start_i)/D(end_i)-1)/alpha_i`, requiring finite positive DFs/alphas and finite L.
3. Set a common displacement `max(0,0.01-min_i L_i)`, so the minimum shifted initial rate is at least1%. Use two constant factors with row `base_vol*[1-0.4*u,0.4*u]`, where `u=original_period_index/original_period_count`. There are no volatility breakpoints. This is a prescribed loading/correlation shape, not a fitted full covariance matrix. Changing curves can change the displacement; rates risk repricing therefore includes that parameter reconstruction.
4. Defaults from the embedded product JSON are 25,000 independent streams, seed42, antithetic=true, degree3 and minimum4 steps/event gap; OOS=false. Unlike HW Bermudan orchestration, this adapter passes its pricer config unchanged and does not apply the instrument mc_paths override. The scalar lmm_base_vol override is required.

### Simulation, payoff and continuation

Build a grid containing exercises, tenor starts and vol breakpoints, plus maturity and the shared monthly subdivision floor. Exercise times must be finite, strictly increasing, more than `1e-10` apart, strictly inside maturity and within `1e-10` of a positive tenor start. Use Philox substreams, negated-shock antithetics and the LMM predictor-corrector. Reject excessive simulated paths and fewer than two independent pricing estimators after any OOS split.

At exercise let j be the first live forward (tenor start plus `1e-8` must not be before t). Reconstruct `P(t,T_k)=product_{i=j}^{k-1}1/(1+alpha_i*L_i)`, `A=sum(alpha_i*P(t,T_{i+1}))`, `S=(1-P(t,T_end))/A`; return S=0 for annuity magnitude at most `1e-15`. Exercise amount is `sign*(S-K)*A*N`. Deflate by the path's terminal bond `P(t,T_end)`; nonpositive terminal numeraire gives zero in the local fallback. Terminal exercise retains positive values.

For earlier exercise regress future chosen **terminal-deflated** cashflow against ITM training states. Basis always includes `[1,S,A]`; degree>=2 adds `[S²,S*A]`, degree>=3 adds `S³`. Higher requested degrees add no further terms but increase the regression count threshold. Fit when ITM training count exceeds `basis_degree+3`, using the shared least-squares solver. Apply exercise if positive deflated immediate value exceeds fitted continuation. **If the ITM count is insufficient, compare immediate value against each path's realized future chosen cashflow and select the larger.** This fallback uses future path information and can introduce foresight bias, including when OOS mode is selected; it is materially different from HW's skip-exercise fallback.

Mean terminal-deflated payoff is multiplied by the time-zero terminal discount factor. Antithetic pairs count as one estimator; optional OOS trains even base streams and prices odd ones. Return PV and positive standard error. The public wrapper finally compares PV with deterministic exercise today, using the shared single-curve HW cashflow helper at zero volatility. Matured/no-future-exercise paths short-circuit before full LMM contract validation, so an exercise-today-only request can bypass the physical/matching-schedule gate.

### Alternatives and risk

Independent fitted vol/correlation term structures, different displaced-rate floors and alternate stopping bases are unimplemented adapter alternatives, even though lower-level LmmParams supports richer inputs. The common Bermudan Delta/Gamma remain discount-curve bump/reprice channels; HwSigmaVega registration is not an LMM volatility sensitivity, and ExpectedExerciseTime is not published by LMM. No dedicated lmm_base_vol sensitivity is published.

Independent market/covariance and out-of-sample regression validation was not established.

## Evidence

- [LMM product adapter](../../../../finstack-quant/valuations/rates/src/swaption/lmm_pricer.rs), [LSMC/numeraire kernel](../../../../finstack-quant/valuations/rates/src/swaption/pricing/lmm_bermudan.rs), [product defaults](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json), [shared metric registration](../../../../finstack-quant/valuations/rates/src/swaption/metrics/mod.rs).
- [Numeraire tests](../../../../finstack-quant/valuations/rates/src/swaption/pricing/tests/numeraire.rs) compare single-exercise terminal-measure estimates and a co-terminal lower bound using shared process primitives. Inline adapter/kernel tests locate contract gates, parameter shape, grid and sampling checks. They were not executed and do not qualify the low-ITM fallback as unbiased.
- [Andersen/Piterbarg](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling), [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) and [original LMM and discretization references](../../../REFERENCES.md#displaced-lmm-and-terminal-bond-pricing) give provenance. The observed factor shape has no independent fit qualification. The existing calibration routes include [Rust](../../../../finstack-quant/calibration/src/lmm.rs), [Python](../../../../finstack-quant-py/src/bindings/calibration/mod.rs) and [WASM](../../../../finstack-quant-wasm/src/api/calibration/mod.rs).

### Independent review evidence

- At an eligible future second exercise, take origin factors
  `.97,.94,.92,.88` from the first tenor through the terminal
  date, the remaining accruals `.5,.75`, strike `.04` and one
  million notional. The deterministic terminal annuity is
  `14/11`, signed payer value per notional in terminal units is
  `19/1100`, and PV is `15,200`. Referencing the numerator to
  the preceding tenor while keeping the same deflator instead
  gives about `14,729.90`.
- For contractual accruals `.5,.6`, initial forwards `−.005,.03`,
  shift `.02` and strike `.015`, the shifted forwards are
  `.015,.05` and both exact support inequalities hold. The first
  factor is about `1.002506266`, and receiver PV per unit notional
  is about `.001162040662`. For original index three of four, the
  selected shape remains `(.7,.3)` after trimming and becomes
  `(.14,.06)` at scale `.2`.
- In the one-terminal-forward case with `F=.023,d=.015,K=.025`,
  one-year horizon, loading norm `.30`, accrual `.5`, terminal
  factor `.95` and one million notional, the ideal shifted Black
  reference gives payer `1,765.79332943`, receiver
  `2,715.79332943` and parity `−950`. It does not test drift
  approximation for interacting forwards.
- With one period, `τ=1,F=2^-54,K=2^-55,N=2^55`, the cash sum gives
  exact terminal exercise `1` despite the rounded binary64
  `1+F` being one. A terminal factor of `.98` gives PV `.98`.
- Two three-period strips with half-year accruals and local bond
  prices `(.98,.95,.90)` and `(.99,.94,.90)` both have
  `A=1.415` and `S=20/283`; their forwards differ. If a sparse
  date has immediate value `4` and equally likely later values
  `2,6`, its nonanticipating value is `4`; the old pathwise
  maximum averages `5`.
- With matched plus leg PV groups `((13,17),(54,60))` and minus
  groups `((12,16),(52,58))`, a represented span of `.02` gives
  paired slope observations `50,100`, mean `75` and standard
  error `25`. An interval computed from two headline errors would
  not include the measured pairing covariance.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
