# VALUATIONS-FRAMEWORK-02 — Volatility, fitted model inputs, drift shape and simulation controls

- Decision status: **Accepted — complete checked valuation model inputs and controls package, 2026-11-13**.
- Implementation status: Accepted target. The observed shared resolution paths appear below.
- Validation status: Source and independent arithmetic examples checked. Runtime, fit replay and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked valuation model inputs and controls

1. **Resolve the actual selected model and input roles.** Use the
   effective valuation close and registered product model admitted
   by [record 01](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).
   Select each active financial input by its typed role and exact
   identity. Resolve and validate the complete input block before
   model execution. Reject incomplete, conflicting, wrong-kind
   and inapplicable supplied controls for the requested operation.
   A validated lifecycle branch can omit lookups for inputs it does
   not use. Record the active branch and effective values. Pricing
   consumes authored parameters or an admitted existing fit.
   Model-owned deterministic preparation, such as fitting the
   short-rate shift to a supplied discount curve, retains its own
   contract. Calibration of missing economic parameters requires
   a separate calibration operation.

2. **Select one scalar or typed market volatility source.** An
   explicit scalar carries finite nonnegative annual volatility,
   its quote family, units and complete model clock, with the
   displacement supplied when shifted Black is selected. The
   scalar branch needs no market volatility lookup. For a market
   quote, select the exact source ID and kind: a strike grid,
   tenor grid, SABR cube or qualified slice, or an FX-delta
   source. Bind the actual underlying, index or currency pair
   and compatible price or rate units. Match strike, tenor and
   delta to their actual roles. A strike-only adapter uses a
   compatible strike grid. An FX-delta adapter requires the
   exact pair and quote profile and the actual positive delivery
   forward. A caplet or other product needs an admitted underlying
   tenor for a tenor or cube query. Reject a dummy tenor or the
   use of a forward as a tenor argument.
   [CORE-MARKET-14](../core-market/volatility-grids-and-shocks.md),
   [CORE-MARKET-15](../core-market/fx-delta-quotes.md),
   [CORE-MARKET-16](../core-market/sabr-cube-data.md).

3. **Keep quote family and displacement exact.** For a volatility
   grid or FX quote source, use its declared convention and
   qualified displacement. Any separately supplied convention or
   shift for the same quote must agree exactly on the admitted
   represented inputs. Unshifted and shifted Black remain distinct
   when the shift is zero. Remove near-match tolerances, convention
   inference from an unrelated artifact and implicit normal-to-Black
   conversion. A SABR parameter cube requires explicit selection of
   the evaluated quote family under its accepted source and model
   domain. Preserve the SABR backbone shift in provenance even
   when the evaluated output is Normal. Normal accepts finite
   signed forward and strike. For Black or shifted Black require
   the actual effective forward and strike, after any displacement,
   to be finite and strictly positive. Zero volatility is valid
   where the product's admitted deterministic branch supports it.
   Keep option-quote volatility distinct from rate-factor or
   stochastic variance parameters.

4. **Apply the dated source clock and supported quote boundary.**
   Resolve the volatility source expiry clock under
   [ADR 0045](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility).
   Use stored calibration metadata or a complete qualified source
   declaration. If both are supplied they agree. Ordinary direct
   pricing requires calibration origin equal to valuation close.
   A different origin requires an explicitly qualified aging or
   recalibration method. Preserve actual expiry, underlying tenor,
   model time and queried coordinates separately. When the
   operation supports an integrated-variance bridge, preserve
   `σ_source² Δt_source = σ_model² Δt_model` on the same
   physical interval. Require positive admitted source and model
   intervals on a live quote path. Use checked in-domain lookup,
   or an explicitly selected extension supported by both source
   and product. Record the effective quote coordinates while
   retaining contractual model time. Actual expiry and other
   branches without a live quote use their product-owned rules.
   Preserve source and model volatility bases for risk so a
   native quote move can be resolved and bridged again, while a
   selected scalar move stays on its model basis.
   [Accepted volatility evaluation](../models-analytic/volatility-source-interpolation.md),
   [Strata's swaption volatility interface](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swaption/SwaptionVolatilities.html).

5. **Resolve one complete Hull–White parameter block.** Select
   one authored product block, authored pricer block or exact
   fitted market block. Choose scalar or scheduled absolute
   short-rate volatility together with mean reversion. Reject
   partial or mixed sources and simultaneous scalar and scheduled
   requests. A scalar-only consumer rejects a selected schedule.
   Bind a fitted block to the exact fit identity and family,
   discount and applicable projection roles, currency, origin,
   model clock and retained calibration provenance under
   [CORE-MARKET-06](../core-market/calibration-provenance.md).
   Keep dimensionless numeric input fields with documented model
   units. A Money market scalar cannot supply a model coefficient.
   Mean reversion is per model year and sigma is absolute decimal
   rate volatility per square root model year. Preserve the
   exact model-time schedule knots, origin, interval ownership
   and tail from the accepted Hull–White and piecewise contracts.
   Do not reconstruct the schedule from rounded calendar dates
   or automatically reanchor it to a new pricing or curve date.
   Use the accepted dated ACT/365F model mapping where applicable.
   Require finite nonnegative mean reversion and sigma.
   Admit exact zero mean reversion and zero sigma where the
   selected kernel supports those limits. Preserve separately
   selected calibration bounds. Curve and parameter conditioning
   or replay requires its own qualified operation.
   [Hull–White kernels](../models-analytic/hull-white-analytic-kernels.md),
   [rate clocks](../models-analytic/rate-clock-and-projection.md),
   [piecewise coefficients](../core-numerics/piecewise-coefficients.md).

6. **Resolve Heston as one coherent pricing-measure block.** Select
   one validated authored or exact fitted `(κ,θ,σ_v,ρ,v_0)`
   block bound to the underlying, relevant market roles, model
   clock and origin. Retire the unqualified global five-scalar
   lookup and refuse mixed or partially resolved blocks. Preserve
   the accepted classical Heston domain of finite `κ>0`,
   `θ>0`, `σ_v>=0`, `v_0>=0` and `−1<ρ<1`. The
   selected transform, PDE or simulation route retains its
   additional model admission, including support for the
   deterministic-variance limit. Feller is a diagnostic.
   Obtain finite signed rates and dividend carry separately on
   the model's admitted basis. Preserve fit provenance and hold
   the block fixed for ordinary market repricing unless the
   experiment explicitly changes it or selects qualified replay.
   [Accepted Heston transform](../models-analytic/heston-characteristic-function.md),
   [Heston 1993](https://doi.org/10.1093/rfs/6.2.327).

7. **Resolve continuous dividend carry explicitly.** Select a
   finite signed annual decimal yield on the admitted model clock
   from an authored input or an exact, qualified unitless source
   binding. Where appropriate, the product can explicitly select
   no continuous carry. A selected missing, monetary or nonfinite
   quote errors. When a model uses discrete cash dividends, the
   product owner determines whether a separately qualified
   residual continuous yield applies, including its scope.
   Preserve each dated cash event once.
   [Dividend events](../core-market/dividend-events.md).

8. **Build curve-based GBM carry from dated relative factors.**
   For same-currency GBM with constant proportional yield `q`,
   select the dated carry checkpoints needed by the product
   through the last underlying fixing. Map their dates to exact
   model knots `t_i`, and obtain relative factors `D_i=D(v,d_i)`
   on the admitted source clock. Set `t_0=0,D_0=1`. The
   cumulative carry is `−log(D_i)−q t_i`, with adjacent
   carry `log(D_(i−1)/D_i)−q(t_i−t_(i−1))`. Use checked
   stable log differences, permitting negative rates. Preserve
   the carry of each interval without requiring an unneeded
   ratio to fit. Use the accepted piecewise-linear cumulative
   schedule between checkpoints. Bind carry breaks and required
   observations to the actual grid. Reject required distinct
   carry dates that collapse to one model time, unresolved
   arithmetic and missing coverage. Remove independently
   generated uniform drift sampling, terminal-rate rescaling
   and its near-zero fallback. Discount the payoff on its
   settlement basis separately from carry to the underlying
   fixing. Flat carry, FX, quanto and explicitly supplied
   forward paths retain their qualified product bases.
   [Accepted GBM](../models-monte-carlo/04-lognormal-diffusions.md),
   [exact grids](../core-numerics/time-grids.md),
   [QuantLib's interval carry](https://github.com/lballabio/QuantLib/blob/master/ql/processes/blackscholesprocess.cpp).

9. **Resolve one complete simulation profile before drawing.**
   For the primary fixed-budget pseudorandom engine require
   the admitted process, scheme, payoff, initial state, grid,
   pricing currency and discount basis, independent estimator count,
   pairing, seed and stream selection and execution controls.
   Apply the accepted inclusive `2..=10,000,000` count
   domain and preflight actual grid, draw, trajectory and
   allocation work. Product resource constraints require an
   explicit owning profile. Reject a zero or out-of-domain
   count before defaults or allocation. Remove the shared
   5,000,000 cap and implicit 100,000-path, annual-step and
   rough-step choices from this generic route. Existing
   LMM Bermudan, Merton PIK and revolver settings may serve
   as versioned product profiles only when separately
   adopted and explicitly selected. Remove the panic loader.
   Preserve product requirements for distinct training and
   pricing counts, exercise grid, regression and QMC controls.
   A selected but unsupported control returns an error.
   An optional supported confidence half-width tolerance is
   finite, positive and in native PV currency. Apply it after
   the fixed run. There is no generic early stopping.
   [Accepted MC execution](../models-monte-carlo/01-engine-and-estimation.md),
   [exercise training](../models-monte-carlo/23-lsmc-exercise.md).

10. **Preserve exact seed inputs and actual work.** Supply an
    explicit seed, including zero, or explicitly choose the
    accepted named seed derivation with instrument identity
    and scenario label. Remove the implicit `base` label.
    Use the selected engine's accepted random profile and
    stream addressing. Report exact seed and stream, count
    of independent estimators and trajectories, actual grid,
    pairing, execution mode and effective chunk rule where
    applicable. Carry full-width integers through Python and
    qualified exact JS and wire inputs under the random stream
    contract. Keep seeds out of binary64 metric fields.
    Preserve paired random assignments where
    the accepted risk experiment requires common random
    numbers. Report the actual selected profiles for
    specialized engines as well.
    [Accepted random streams](../models-monte-carlo/02-random-streams.md),
    [paired Monte Carlo risk](../models-monte-carlo/25-finite-difference-greeks.md).

11. **Keep shared tree, rate and quote controls in their own units.**
    Require an explicit short-rate model for the active
    rates-only bond tree and checked positive steps or its
    qualified dated grid. Use Hull–White absolute rate
    volatility or BDT/BK annual log-rate volatility as
    selected. BDT has zero mean reversion by law. BK
    requires strictly positive mean reversion and its
    positive-rate fit domain. Exact zero volatility uses
    the selected owner's deterministic case. Keep the joint
    rates-credit lattice separate, with absolute additive
    hazard volatility and explicit mean reversion. Its
    correlation is required and feasible for two stochastic
    axes and absent for a deterministic axis under the
    accepted joint owner. Keep a positive finite LMM
    loading scale only on its admitted route. Call friction
    is finite nonnegative cents per 100 of outstanding par
    and affects the exercise threshold. Redemption uses
    the contractual terms. Resolve curve roles explicitly.
    Require the bond owner's admitted OAS quote compounding
    (continuous or supported semiannual periodic, with
    narrower model-specific restrictions). Keep OAS quote
    compounding separate from base tree discount compounding.
    [BDT](../models-numerical/bdt-lognormal-curve-fit.md),
    [BK](../models-numerical/black-karasinski-trinomial.md),
    [joint lattice](../models-numerical/rates-credit-coupling-and-sampling.md),
    [bond spread quotes](../valuations-bonds/12-spread-quotes.md#accepted-decision-required-quote-compounding).

12. **Use one Rust admission and provenance path.** Validate
    construction and restore against the selected contract.
    Report requested controls separately from actual source
    bindings, effective model and clock, curve roles, parameter
    block, boundary policy and execution profile. Preserve
    original calibration history without presenting a scenario
    as a new fit. Reconcile existing product, calibration,
    scenario and risk callers, Rust and host declarations,
    schemas, generated artifacts and parity. Python and WASM
    forward the same choices and errors. The calibration owner
    retains objective, fit acceptance and solver choices.

## Observed behavior

### Choice, scope and algorithm

Shared helpers resolve the economic inputs before the selected product model runs. They do not calibrate a missing model during pricing.

1. Resolve a volatility source from the market (cube, then ordinary surface, then FX-delta surface). An explicit scalar implied-volatility override plus explicit model convention may work without a source; automatic convention selection still needs source metadata. A present source supplies its expiry/tenor convention. Explicit lognormal with a shifted source retains the source shift; two explicit shifts differing by more than `1e−12` fail. An explicit normal model reads normal quotes, while lognormal/shifted models read Black quotes. This is model-aware retrieval, not arbitrary quote conversion. Sigma must be finite and nonnegative. Normal accepts finite signed forward/strike; Black requires both displaced coordinates strictly positive. The equity/FX `resolve_sigma_at` helper requires a strike-axis surface when no scalar override is active, clamps lookup, and rejects displaced sources for its unshifted consumer.
2. Scalar Hull–White resolution requires a complete positive finite `(kappa,sigma)` pair. Instrument override wins, then a complete constructor-supplied pair, then market scalars named by curve and cap/floor versus swaption family. Partial overrides fail even when a complete fallback exists. A volatility schedule fails in this scalar-only path. Market scalars can be unitless or monetary numeric values; invalid or absent values produce missing/partial-parameter errors. This differs from the explicit deterministic-zero permission of particular bond lattices. No volatility surface fitting or default short-rate parameters occur here.
3. Closed-form Heston resolution requires all five unitless market scalars `HESTON_KAPPA`, `HESTON_THETA`, `HESTON_SIGMA_V`, `HESTON_RHO`, `HESTON_V0`; missing or monetary entries fail. Caller-supplied continuous rates and dividend yield are combined through the model constructor, which owns parameter admissibility.
4. Optional dividend yield defaults to zero only when no identifier was supplied. A configured missing or monetary scalar fails. The helper itself does not independently reject nonfinite unitless yield; downstream model validation is relevant.

### Term-structure drift adapter

For GBM paths, sample at `max(num_steps,1)+1` evenly spaced knots over `[0,T]`. Let `u0` be the signed curve time at valuation and `D0=DF(u0)`, which must be positive finite. Form `C(t)=−ln[DF(u0+t)/D0]`. If `|C(T)|>1e−12`, multiply the curve shape by `a=rT/C(T)`; otherwise set `a=1`. The cumulative drift is `M(t)=aC(t)−qt`. The model's drift-schedule constructor validates the result. This retains term-structure shape while matching the supplied maturity-effective rate when the scaling denominator is usable. It samples additive model time on the curve clock, not a date for every interior knot. When `|C(T)|≤1e−12`, terminal matching is not forced if supplied `rT` differs.

### Simulation and numerical controls

The common path helper defaults to 100,000 paths, 252 steps/year, and 100 steps for rough-volatility adapters, with an allocation ceiling of 5,000,000 paths. Actual pricers can supply different defaults through the embedded pricer registry. `resolve_mc_paths` treats absent/zero override as the caller default and rejects the resulting count above the ceiling; its direct default argument can itself be zero. In contrast `ModelConfig::validate` rejects an explicit zero `mc_paths`. The merged path configuration changes only path count and optional antithetic flag. Seed derivation uses instrument ID plus scenario label (default `base`); product-specific consumption of CI targets and other settings is not implied by their presence on the shared config.

Model configuration defaults to clamped volatility extrapolation and no explicit tree model; callers interpret no tree model as Hull–White. Tree steps must be nonzero; shared volatility, mean-reversion and call-friction fields are finite nonnegative; LMM loading scale and CI target must be positive finite; rate-credit correlation lies in `[-1,1]`. Product paths impose stricter combinations. HW sigma is absolute annual short-rate volatility; BDT/BK sigma is relative log-rate volatility; option implied volatility is a different quote. Call friction is cents per 100 par and changes the exercise decision, not contractual redemption. The live shared OAS validator admits continuous and semiannual periodic only. Some product paths narrow this further.

The inspected [product-default JSON](../../../../finstack-quant/valuations/core/data/defaults/product_mc_defaults.v1.json) sets LMM Bermudan to 25,000 paths, seed 42, degree-three basis, antithetics and at least four steps between exercises; Merton PIK to 5,000 estimators, antithetics and 12 steps/year; revolver to 10,000 estimators without antithetics. Loading verifies contract/version plus positive LMM path/basis/step counts, positive Merton paths/grid density and at least two revolver estimators. The separate `embedded_defaults_or_panic` entry treats corrupted embedded assets as a panic; the fallible loader returns validation errors. These values do not imply equal estimator meanings or effective seed policy across every consuming engine.

### Alternatives, evidence

Direct surface calibration, automatic scalar unit conversion, flat maturity drift, and automatic model substitution are distinct methods; only the explicitly described branches are implemented here.

- [vol_resolution.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/vol_resolution.rs), [market volatility bridge](../../../../finstack-quant/valuations/core/src/market/volatility.rs), [hw1f_params.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/hw1f_params.rs), [heston_market.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing/heston_market.rs), [helpers.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/helpers.rs), [pricing_overrides.rs](../../../../finstack-quant/valuations/core/src/instruments/pricing_overrides.rs), and [defaults.rs](../../../../finstack-quant/valuations/core/src/pricer/defaults.rs) own these policies. Inline tests cover missing/partial inputs, scalar overrides, axes and schedule rejection; this is behavioral evidence, not an independent numerical benchmark.
- [Glasserman](../../../REFERENCES.md#glasserman-2004-monte-carlo) supplies simulation context. The default path counts and drift adapter are implementation choices awaiting agreement.

### Independent review evidence

- A 20% ACT/360 source quote over 365 actual days carries total
  variance `.04055555555555556`. It becomes about `20.13841%`
  on an ACT/365F model clock. Leaving sigma at 20% on that clock
  gives variance `.04`. Separately, adding finite binary64
  maximum forward and shift produces infinity, exposing the
  need to check the displaced coordinate after addition.
- For a one-year Hull–White model with `κ=.05`, sigma `.01`
  before `.25` years and `.02` after, the one-year factor
  variance is about `.000311932122978382`. Moving the knot
  to the rounded day coordinate `91/365` changes it to
  `.000312122748673248`.
  [Observed calibration writer](../../../../finstack-quant/calibration/src/step_runtime.rs)
  and [cap/floor resolver](../../../../finstack-quant/valuations/rates/src/cap_floor/hw_pricer.rs).
- Take a curve on ACT/360 and a GBM model on ACT/365F, with
  a 365-day terminal fixing, spot 100 and zero dividend yield.
  On the curve clock set `U=365/360`, `C(u)=.04(u/U)(1−u/U)`
  and `D(u)=exp(−C(u))`. The dated terminal carry is zero.
  At day 182 the dated factor implies expected spot about
  `101.00500912688`. The observed additive-time rescaling
  uses a nonzero source `C(1)` and a zero terminal rate,
  flattening the profile to 100 at the interim fixing.
- Exact seed `9007199254740993` becomes `9007199254740992`
  through binary64. Ten million independent estimators under
  antithetic pairing require twenty million trajectories
  before grid and storage checks. Call friction of 50 cents
  per 100 of par gives a threshold increment of 5,000 on
  one million outstanding.
  [Observed Bermudan seed metric](../../../../finstack-quant/valuations/rates/src/swaption/bermudan.rs).

Production qualification includes typed source collision cases,
dated lookup and fit replay, exact schedule restoration, actual
carry and payoff grid bindings, control consumption by each engine,
and native and host provenance and error parity.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
