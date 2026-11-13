# Monte Carlo implementation methodology audit

Decision status: **All 28 complete packages accepted.** Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

Scope is exactly **78 assigned Rust files** under `finstack-quant/models/src/monte_carlo`: 69 algorithm-bearing, 5 support, 4 test-only and 0 generated. The [coverage ledger](coverage.json) lists every assigned file exactly once, with concrete responsibilities and methodology links. The 28 records cover simulation, numerical controls, payoff rules and estimator choices.

Production bodies were inspected, including runtime code after inline test markers. Selected standalone/inline tests were read and other named validation cases located. The original audit ran no tests, lint, build or numerical experiments. The versioned runtime-default asset was inspected as a material dependency outside the Rust file count. [Runtime-default asset](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json).

Core RNG/linear algebra/statistics/time-grid kernels, volatility-surface construction, shared Hull–White parameters and product-specific valuations callers are dependencies referenced where needed; their owning algorithms belong to other slices. This slice includes their selection and use, all embedded calibration in its source, and all material fallbacks found here.

The first complete package was accepted on 2026-11-11. It selects a
fixed-budget, checked pseudorandom pricing engine, one positive scalar
discount, explicit run inputs, independent estimator counts and a fixed-sample
95% interval. Runtime reconciliation, host parity and financial qualification
are pending. [Accepted record 01](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

The second complete package was accepted on 2026-11-11. It selects a
pinned Philox profile, exact open midpoints, fallible draws, cache and
exhaustion rules, and explicit stable seed derivation.
[Accepted record 02](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

The third complete package was accepted on 2026-11-11. It selects
equal dyadic Sobol blocks, checked direct or bridge construction and
descriptive replicate spread.
[Accepted record 03](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

The fourth complete package was accepted on 2026-11-11. It selects
checked exact GBM, bounded cumulative carry, a dated ATM forward-variance
bootstrap and checked log-Euler local volatility.
[Accepted record 04](04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

The fifth complete package was accepted on 2026-11-11. It selects
checked coordinatewise Euler, retirement of obsolete generic log-Euler
and Milstein branches, and exact additive Brownian and multi-OU steps.
[Accepted record 05](05-generic-steppers.md#accepted-decision-for-checked-coordinatewise-euler-and-exact-gaussian-steps).

The sixth complete package was accepted on 2026-11-11. It selects
capped cash and proportional jumps on exact knots, explicit ex phases,
a qualified delayed-cash adapter and one normal per actual interval.
[Accepted record 06](06-cash-dividend-jumps.md#accepted-decision-for-checked-discrete-dividend-jumps-on-exact-grid-knots).

The seventh complete package was accepted on 2026-11-11. It selects
checked exact Hull–White endpoints for explicit mean levels and retirement
of curve-to-theta fitting in dated callers.
[Accepted record 07](07-hull-white.md#accepted-decision-for-checked-hullwhite-endpoints-and-curve-fit-boundaries).

The eighth complete package was accepted on 2026-11-11. It selects
checked CIR and CIR++ QE, log-tail sampling, a shared log moment and
an unshifted initial factor with checked signed CIR++ rate output.
[Accepted record 08](08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).

The ninth package was accepted on 2026-11-11 for checked classical Heston
QE-M on the actual grid, strict moment errors and the exact
deterministic-variance limit.
[Accepted record 09](09-heston-qe.md#accepted-decision-for-checked-heston-qe-m-paths).

The tenth package was accepted on 2026-11-11 for checked terminal-measure
LMM predictor-corrector, strict displaced support, exact fixing knots
and explicit tenor-bond ratios.
[Accepted record 10](10-lmm.md#accepted-decision-for-checked-terminal-measure-lmm-paths).

The eleventh package was accepted on 2026-11-11 for checked revolving-credit
steps, clamped Gaussian utilization, literal constant spread, latent normal
correlation and explicit joint Hull–White discounting.
[Accepted record 11](11-credit-line.md#accepted-decision-for-checked-revolving-credit-paths).

The twelfth package was accepted on 2026-11-11 for checked exact
Schwartz–Smith paths, explicit factor initialization and a stable
conditional spot expectation under the supplied law.
[Accepted record 12](12-schwartz-smith.md#accepted-decision-for-checked-schwartzsmith-paths-and-expectations).

The thirteenth package was accepted on 2026-11-11 for checked standalone
fBM and one actual-grid Volterra driver for the named rough models, with
explicit Brownian leverage and sampled node variance.
[Accepted record 13](13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).

The fourteenth package was accepted on 2026-11-11 for checked adapted
rough Bergomi, exact variance break alignment, sampled-grid normalization
and bounded European call estimation by parity.
[Accepted record 14](14-rough-bergomi.md#accepted-decision-for-checked-rough-bergomi-spot-and-pricing).

The fifteenth package was accepted on 2026-11-11 for checked rough Heston
on the actual grid, full Gamma-normalized history, explicit variance
projection and direct European estimation on a qualified finite grid.
[Accepted record 15](15-rough-heston.md#accepted-decision-for-checked-rough-heston-paths-and-pricing).

The sixteenth package was accepted on 2026-11-11 for checked rough
Cheyette raw paths with an explicit amplitude and shift, sampled RL
normalization, exact frozen state marginals and a specified within-cell
covariance approximation.
[Accepted record 16](16-rough-cheyette.md#accepted-decision-for-checked-rough-cheyette-paths).

The seventeenth package was accepted on 2026-11-11 for checked terminal
and Asian payoffs with complete observation coverage, explicit
count-normalized fixing rows and separate diagnostic and settlement values.
[Accepted record 17](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

The eighteenth package was accepted on 2026-11-11 for checked barrier
monitoring and rebates with unshifted GBM bridges, conditional first-passage
times, an explicit Heston approximation and same-source cash discounting.
[Accepted record 18](18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).

The nineteenth package was accepted on 2026-11-11 for checked lookback
extrema with exact node or GBM bridge monitoring, qualified history and
terminal fixing semantics, and stable scaled extremum arithmetic.
[Accepted record 19](19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema).

The twentieth package was accepted on 2026-11-11 for checked autocallable
payoffs with exact observations, independent coupon and call tests, the
three redemption formulas and actual dated cash for replay and live pricing.
[Accepted record 20](20-autocallable.md#accepted-decision-for-checked-monte-carlo-autocallable-payoffs).

The twenty-first package was accepted on 2026-11-11 for checked cliquet
payoffs with exact reset nodes, explicit additive or multiplicative
aggregation, and shared historical and live settlement semantics.
[Accepted record 21](21-cliquet.md#accepted-decision-for-checked-monte-carlo-cliquet-payoffs).

The twenty-second package was accepted on 2026-11-11 for checked range
accrual with exact observation rows, certified history, a fixed count,
checked Money arithmetic and one coupon on its actual payment date.
[Accepted record 22](22-range-accrual.md#accepted-decision-for-checked-monte-carlo-range-accrual).

The twenty-third package was accepted on 2026-11-11 for exact GBM
exercise knots, explicit sparse-date and origin decisions, immutable
replay and held-out policy value with conditional pricing uncertainty.
[Accepted record 23](23-lsmc-exercise.md#accepted-decision-for-checked-holder-exercise-policy-training-and-pricing).

The twenty-fourth package was accepted on 2026-11-11 for explicit holder
and multivariate bases, frozen scaling, bounded full-rank SVD and
checked prediction with owner-defined state and response units.
[Accepted record 24](24-regression-policies.md#accepted-decision-for-checked-continuation-regression).

The twenty-fifth package was accepted on 2026-11-11 for actual spot nodes,
complete shared-draw groups, checked signed stencils and fixed-sample
Student-t uncertainty.
[Accepted record 25](25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks).

The twenty-sixth package was accepted on 2026-11-11 for qualified
density-only payoffs, scores from consumed shocks, streamed complete
groups and native Greek units.
[Accepted record 26](26-likelihood-ratio-greeks.md#accepted-decision-for-checked-gbm-likelihood-ratio-greeks).

The twenty-seventh package was accepted on 2026-11-11 for matched control PV,
independent coefficient fitting, complete paired groups and
held-out adjusted uncertainty.
[Accepted record 27](27-control-variate.md#accepted-decision-for-checked-single-control-monte-carlo-pricing).

The twenty-eighth package was accepted on 2026-11-11 for exact retained
membership, timed diagnostic cash, explicit model-owned schemes and
checked replay.
[Accepted record 28](28-capture-and-simulation-contracts.md#accepted-decision-for-checked-path-capture-and-raw-simulation).

## Methodology inventory

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation | Scope and evidence limits |
|---|---|---|---|---|---|
| Generic payoff pricing | Time-zero/step events, scalar discount, process correlation, dedicated-scheme validation | Caller composes process/scheme; convenience functions fix GBM/QE | [Accepted 01](01-engine-and-estimation.md) | Engine runtime inspected; deterministic-reduction and error-path tests | Accepted positive scalar discount and checked event/leg admission |
| Sampling error and antithetic pricing | Pair means; online variance; Student-t CI; optional stopping after 5,000 samples | Antithetic false by generic default; stopping explicit and serial | [Accepted 01](01-engine-and-estimation.md) | Pair counts and stopping-count tests inspected; independent examples checked | Accepted fixed-budget inference and removal of generic auto-stop |
| Omitted runtime settings | Versioned JSON path/seed/grid/currency defaults | Registry values; host convenience differs from Rust parallel defaults | [Accepted 01](01-engine-and-estimation.md) | Registry asset and constructor resolution inspected | Accepted explicit effective pricing controls |
| Pseudorandom paths and scenario seeds | Philox4x32-10, open uniforms, Box–Muller cache, delimited FNV seed hash | Philox used by built-in pricers/simulator | [Accepted 02](02-random-streams.md) | Random123 and seed examples checked independently; runtime qualification pending | Accepted exact addresses and midpoints, fallible draws, exhaustion and explicit seed selection |
| GBM randomized QMC | Whole-path Sobol normals, optional Brownian bridge, up to 16 scramble means | Sobol flag implies bridge; serial and unpaired | [Accepted 03](03-randomized-qmc.md) | Source, primary references and independent block/bridge examples checked | Accepted equal dyadic blocks, explicit construction and descriptive replicate spread |
| Single/multiasset GBM | Exact lognormal transition; external Cholesky; integrated drift schedule | Default GBM path simulation; generic alternatives explicit | [Accepted 04](04-lognormal-diffusions.md) | Direct log moments and correlation formulas inspected | Accepted typed exact transitions, bounded carry and checked range |
| Curve-implied interval GBM | DF-ratio forward rates and total-variance differencing; boundary-selected interval | Lower-level bootstrap/stepper explicit | [Accepted 04](04-lognormal-diffusions.md) | Bootstrap source and independent arithmetic checked | Accepted exact knots, source clocks and ATM variance |
| Local-volatility paths | Surface sigma(t,S), constant carry, log-Euler frozen coefficients | Default log-Euler; Euler override; no proportional Milstein | [Accepted 04](04-lognormal-diffusions.md) | Flat/skew surface tests located; independent examples checked | Accepted qualified grid and checked log-Euler |
| Gaussian/additive and generic diffusion paths | Euler, log-Euler, proportional Milstein; correlated additive/multi-OU | Euler default for Brownian/multi-OU | [Accepted 05](05-generic-steppers.md) | Source and independent Euler, Brownian and OU arithmetic checked | Accepted checked Euler and exact additive Brownian and multi-OU steps |
| Discrete-dividend equity paths | Independent exact diffusion subintervals then cash/proportional jumps | Mandatory dividend scheme; only default in serialized dispatch | [Accepted 06](06-cash-dividend-jumps.md) | Source and independent grid, claim-value and depletion checks | Accepted exact knots, post-ex observations, capped stock drops and one draw per interval |
| Hull–White short-rate transition | Exact exponentially weighted theta and integrated sigma variance | Default exact; Euler explicit | [Accepted 07](07-hull-white.md) | Source and independent endpoint, exact-grid and range checks | Accepted checked exact endpoints and explicit theta schedule |
| Hull–White curve fit | Finite-difference forwards; shift-matching piecewise theta | Explicit scalar/piecewise-sigma fitting helpers | [Accepted 07](07-hull-white.md) | Curve-to-theta callers traced; independent discount example checked | Accepted retirement of the theta fit and reconciliation to the dated shift |
| CIR/CIR++ factor and shifted rates | Moment-matched QE quadratic or atom/exponential regimes | Default QE psi threshold 1.5; Euler override | [Accepted 08](08-cir-qe.md) | Source and independent moment, tail and shift checks | Accepted checked QE, numerical atom and signed rate projection |
| Heston spot/variance paths | QE variance, endpoint-based variance integral and moment correction | Default trapezoidal QE, optional Euler | [Accepted 09](09-heston-qe.md) | Source, primary references and independent pole, leverage, deterministic-limit and carry examples checked | Accepted checked QE-M, exact zero-vol-of-vol limit and strict moment errors |
| Displaced LMM forwards | Terminal-measure drift, log predictor-corrector, fixing freeze and relative bond product | Dedicated scheme, two or three factors | [Accepted 10](10-lmm.md) | Source, primary references, independent step, bond-ratio and bias examples checked | Accepted strict support, exact fixing and loading knots, checked tenor-bond ratios |
| Revolving-credit state dynamics | Clamped Gaussian utilization, Hull–White/fixed/forward rate and CIR spread | Dedicated scheme in the product, generic pairing still possible | [Accepted 11](11-credit-line.md) | Source, primary references, independent clamping, range and covariance examples checked | Accepted checked joint steps, explicit modes and latent coupling |
| Commodity paths and expectation | Schwartz–Smith exact OU/Brownian transition and Gaussian expectation | Default exact, optional Euler | [Accepted 12](12-schwartz-smith.md) | Source, primary references and independent covariance, expectation and pin examples checked | Accepted exact-only checked paths, stable log variance and explicit measure boundary |
| Fractional Brownian increments | Full covariance Cholesky or finite-window conditional recursion | Raw model alternatives with automatic window sizing | [Accepted 13](13-fractional-noise.md) | Source and independent window covariance checked | Accepted checked standalone Cholesky and retirement of finite-window fBM |
| Rough-model RL driver | Exact newest-cell covariance and averaged older cells | Default nominal uniform Volterra with Brownian leverage | [Accepted 13](13-fractional-noise.md) | Source, primary references, independent cell, variance and grid examples checked | Accepted actual-grid RL, sampled node variance and explicit draws |
| Rough Bergomi spot | Left-point variance, log-Euler spot and accumulated history | Dedicated injected-noise scheme | [Accepted 14](14-rough-bergomi.md) | Source, primary references, independent limit, moment and option checks | Accepted checked left-point dynamics and bounded European call estimator |
| Rough Heston spot/variance | Full Volterra history, newest-cell Gaussian residual and positive-part variance | Dedicated hybrid | [Accepted 15](15-rough-heston.md) | Source, primary references, independent kernel, projection and option checks | Accepted exact grid binding, explicit boundary and direct fixed-grid estimation |
| Rough Cheyette short rate | Compensated volatility, Euler x/y and signed short-rate shift | Dedicated injected-noise scheme | [Accepted 16](16-rough-cheyette.md) | Source, primary references and independent frozen step, covariance and projection checks | Accepted checked raw paths and explicit grid approximation, with pricing separately qualified |
| European/digital/forward cashflows | Maturity-step intrinsic, strict digital indicator and signed forward | Explicit payoff types and external scalar discount | [Accepted 17](17-vanilla-and-asian.md) | Source, fixed-path tests and independent settlement and range examples | Accepted immutable terms and complete-only settlement |
| Asian averaging | Kahan arithmetic or log-geometric average, historical aggregates and repeated fixing multipliers | Convenience routes infer arithmetic and the fixing schedule | [Accepted 17](17-vanilla-and-asian.md) | Source, selected tests and independent tied-grid payoff and control examples | Accepted explicit fixing rows and history, count normalization and separate diagnostics |
| Barrier path condition/rebate | Inclusive touch, discrete observation or sampled log bridge, and receiving-node rebate timing | Current path variance overrides scalar sigma for the bridge | [Accepted 18](18-barrier-monitoring.md) | Source, primary references and independent crossing, timing and price checks | Accepted exact event binding, explicit Heston approximation, conditional first passage and one discount source |
| Fixed/floating lookback | Node extrema or conditional lognormal bridge, with historical extrema | Low-level continuous label defaults to node sampling and the asset GBM route enables the bridge explicitly | [Accepted 19](19-lookback-monitoring.md) | Source, selected tests and independent conditional extrema, scaled payoff and price checks | Accepted exact monitoring and history, complete settlement and qualified GBM cells |
| Autocall coupons/redemption | Coupon before call, memory release, payment ratios and three final payoff formulas | Parallel contract vectors and tolerant event matching | [Accepted 20](20-autocallable.md) | Source, filed issuer terms and independent event, redemption and payment checks | Accepted exact observation rows, shared replay and dated cash |
| Cliquet returns | Locally clipped additive or multiplicative returns, then global limits | Additive default, tolerant reset matching and a separate dated known-cash branch | [Accepted 21](21-cliquet.md) | Source, primary references and independent seasoned, range and price checks | Accepted exact resets, shared locked state and complete dated settlement |
| Range-accrual coupon | Inclusive range, equal consumed counts and dated carry multipliers | Tolerant schedule matching and mutable observation dates | [Accepted 22](22-range-accrual.md) | Source, issuer terms and independent count, carry and price checks | Accepted exact tied rows, fixed contractual denominator and checked dated settlement |
| American/Bermudan exercise | ITM backward regression, sparse-date skip, time-zero floor/decision | GBM LSMC with every-step convenience | [Accepted 23](23-lsmc-exercise.md) | Source and independent exact-grid, sparse and two-date price examples | Accepted exact nodes, explicit continue-only coverage rule and frozen origin |
| Independent exercise-policy valuation | Freeze coefficients then replay on distinct-seed paths | Explicit price_unbiased or fit/replay APIs | [Accepted 23](23-lsmc-exercise.md) | Seed and replay tests inspected and look-ahead examples checked | Accepted disjoint Philox groups, complete policy identity and conditional pricing uncertainty |
| Continuation regression basis/solver | Raw or normalized powers, Laguerre and truncated SVD | Caller selects basis, with Laguerre degree 3 convenience | [Accepted 24](24-regression-policies.md) | Basis equivalence, singular-rank and replay examples checked | Accepted strike-centered holder powers and bounded full-rank SVD |
| Multifeature policy approximation | Sample standardization, near-constant feature removal and candidate reduction | Explicit standardized policy fit | [Accepted 24](24-regression-policies.md) | Shifted-scale, interaction and dependent-column examples checked | Accepted explicit feature schema, degree and frozen transforms |
| MC spot delta/gamma | Nominal spot stencils, paired draws and normal interval | Explicit 1D estimator, with default 1% convenience bump | [Accepted 25](25-finite-difference-greeks.md) | Source and independent geometry, pairing and GBM references checked | Accepted actual nodes, fixed-input spot sensitivity and paired Student-t uncertainty |
| MC GBM delta/vega | First-transition delta and summed vega scores reconstructed from captured paths | Rust payoff opt-in, with arithmetic Asian convenience | [Accepted 26](26-likelihood-ratio-greeks.md) | Source and independent score, native-unit and pairing checks | Accepted consumed shocks, single-pass scoring and qualified density-only payoffs |
| Control-variate adjustment | Same-sample covariance/variance beta, adjusted variance floor, normal interval | Shared helper used by dated arithmetic Asian GBM | [Accepted 27](27-control-variate.md) | Source and independent bias, pairing and seasoned GBM checks | Accepted matched control PV, independent pilot and held-out uncertainty |
| Captured-path diagnostics | Expected-count hash sampling, subset quantiles and periodic amount-only IRR | Capture explicit, incompatible with antithetics | [Accepted 28](28-capture-and-simulation-contracts.md) | Source and independent capture, subset and timed-cash examples | Accepted exact membership, storage cap and diagnostic isolation |
| Public path simulation | Process/scheme dispatch, raw coordinates and storage guards | Serializable default and override scheme selections, with Philox streams | [Accepted 28](28-capture-and-simulation-contracts.md) | Source and independent knot, count and resource checks | Accepted explicit model-owned schemes, canonical output and checked replay |

## Evidence and decision limits

Direct moment/reference-vector checks, comparisons against other production methods, fixed-path contractual examples, and roundtrip/invariant tests are distinguished in each record. None substitutes for approved economic assumptions or product-wide numerical error budgets. Adoption questions, implementation reconciliation and validation work are owned by the [Monte Carlo follow-ups](../FOLLOW_UPS.md#monte-carlo-models). Records retain observed behavior, material limitations and evidence boundaries.
