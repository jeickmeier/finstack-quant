# MODELS-MONTE-CARLO-27: Control-variate coefficient and adjusted uncertainty

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing aggregate helper and dated arithmetic Asian production use observed. Checked paired pricing, independent coefficient fitting and caller reconciliation are pending.
- Validation status: Source, selected tests, primary references and independent bias, variance and seasoned Asian examples reviewed. Runtime, product and host qualification are pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and coverage: [slice inventory](README.md), [file ledger](coverage.json).

## Accepted decision for checked single-control Monte Carlo pricing

1. **Select one control explicitly.** Each trajectory supplies
   a target cash value and one control cash value from the same
   underlying path. Require explicit immutable payoff definitions,
   one pricing currency and a qualified expected control PV
   `mu_C` at the same valuation reference. The owner supplies each
   payoff's compatible settlement representation and discount.
   Complete each payoff and apply its checked positive scalar
   discount once under the Money rules. Validate currency even
   for zero cash. Preserve signed amounts. Plain target pricing
   is an explicit selection.
   [Accepted Money](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity),
   [accepted engine](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

2. **Bind the centering value to the pricing law.** Require
   the control identity, model, observable schedule, history,
   units and settlement basis used to compute `mu_C`. A known
   expectation can be signed or zero. It is distinct from a
   sample mean, and a missing, mismatched or unqualified
   expectation fails the controlled operation. If a frozen
   coefficient is applied with a centering value that differs
   from the actual control expectation, the conditional target
   shifts by `beta*(mu_C-E[C])`. An analytic ideal-law value has
   separate finite-random-grid, path and monetary rounding limits.
   The sampling interval does not account for centering error.
   [Control-variate references](../../../REFERENCES.md#paired-control-variates-and-independent-pilots).

3. **Freeze a signed coefficient before pricing.** A Rust
   owner can supply a qualified fixed finite `beta`, including
   literal zero. A data-fitted coefficient uses one explicitly
   sized independent pilot and is frozen before pricing. Use the
   same target/control law, grid, terms, centering value and
   pairing in both phases. The dated controlled Asian route
   uses the pilot for its fitted coefficient. No same-sample
   fit, implicit pilot size, refit, cross-fit or multi-control
   regression is selected.
   [Independent pilot reference](../../../REFERENCES.md#paired-control-variates-and-independent-pilots).

4. **Use explicit fixed-budget Philox groups.** Require 2
   through 10,000,000 independent groups in each used stage.
   In a composed run, one root seed assigns the pilot IDs
   `[0,N_pilot)` and pricing IDs `[N_pilot,N_pilot+N_price)`.
   A supplied fixed coefficient has no pilot draws and pricing
   starts at group zero. Validate the actual grid, payoff events,
   addresses, draw layouts, work and storage before execution.
   Use the accepted ordered serial or native parallel reduction
   and reject unsupported WASM parallel. Reject adaptive stopping,
   capture, and Sobol or Brownian-bridge QMC for this controlled
   operation. Do not substitute plain pricing on failure.
   [Accepted Philox](02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation),
   [accepted Sobol boundary](03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

5. **Build one paired observation per independent group.**
   Observe target and control on each complete leg using the
   admitted shared path and any declared fixed draw slots.
   With antithetics, apply the discounts to each target and
   control leg first, then take the two checked component
   means. Pilot fitting and pricing both use these group
   observations. Do not pool individual antithetic legs as
   separate observations. Reject a failed leg or missing payoff
   with stage, group, leg and operation context. Keep bounded
   path and reduction state and publish no partial estimate.

6. **Fit from checked paired pilot statistics.** Use the core
   scale-normalized covariance-to-control-variance ratio
   `beta=C_xy/M2_y` on complete pilot groups. Require a
   genuinely varying pilot control. A constant pilot leaves
   the coefficient unidentified and fails the selected fit.
   Use shifted/scaled group observations so a common large
   stored PV does not erase a resolved small variation.
   Admit a resolved nonzero variance at any scale and preserve
   a fitted zero or negative coefficient. Remove the absolute
   variance cutoff, free aggregate-moment adjustment entry
   point and covariance or variance repair. Derive the state
   from the validated paired observations.
   [Accepted covariance and beta](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).

7. **Adjust the held-out observations directly.** For each
   pricing group use `Z_i=X_i-beta*(C_i-mu_C)`.
   Preserve stored PV differences before binary64 moment
   updates with checked widened and shifted/scaled arithmetic.
   Keep the scale through the complete signed correction.
   An intermediate difference or product need not be
   representable as Money. Fail on unresolved required
   arithmetic. Preserve literal zero, negative adjustments
   and negative estimates. A constant realized pricing
   control is admissible under a frozen coefficient and can
   still shift the sample. Use the actual adjusted observations
   for variance, removing the aggregate quadratic variance
   expression and its floor.

8. **Report conditional fixed-sample uncertainty and actual
   work.** Use the pricing mean, sample SD with divisor
   `N_price-1`, standard error and two-sided 95% Student-t
   interval. A constant adjusted sample has a point interval.
   Reject incomplete samples and unrepresentable required
   outputs without NaN or saturation. Final Money conversion
   uses the accepted pricing boundary. Report the coefficient
   and source, control expectation provenance, grid, seed,
   pairing, execution and actual pilot and pricing group
   and trajectory counts. Pilot observations are excluded
   from the estimate and its degrees of freedom.
   [Accepted uncertainty](01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

9. **State the qualified variance claim.** With a correctly
   matched expectation and independent pricing, the conditional
   target is the unadjusted population mean for any frozen
   finite coefficient. With finite second moments and varying
   control, the population coefficient minimizing variance is
   `Cov(X,C)/Var(C)` for the selected group law. A finite
   pilot or a supplied coefficient can increase variance.
   Exact Student-t coverage has the accepted IID normal
   conditions. Conditional uncertainty describes the adjusted
   distribution for the realized coefficient. Qualify pilot
   variability, rare controls, model error and efficiency
   including pilot cost separately. No universal path-count
   threshold makes same-sample coefficient bias negligible.
   [Control-variate references](../../../REFERENCES.md#paired-control-variates-and-independent-pilots).

10. **Match the dated Asian geometric control exactly.**
    Reuse the accepted geometric moment law for the arithmetic
    target's actual GBM grid and geometric control payoff.
    Match carry, constant volatility and covariance, certified
    positive control history, ordered fixing rows and
    multiplicities, exact times and multipliers, spot, strike,
    direction, quantity, currency and the actual payment
    factor. A qualified origin row contributes its fixed
    time-zero level to both representations. Do not
    deduplicate tied rows or substitute the last fixing's
    discount for payment discount. Reuse the checked
    analytic kernel and retire the duplicate mean formula.
    Incompatible history, model or expectation fails the
    selected control. Known fully observed settlement and
    product-level model selection stay with the dated owner.
    [Accepted Asian payoffs](17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs),
    [accepted analytic moments](../models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing).

11. **Reconcile the live consumer and existing host boundaries.**
    Replace same-sample fitting in the dated arithmetic Asian
    GBM pricer with the checked pilot and held-out operation.
    Its owner makes plain versus controlled selection explicit
    and supplies pilot and pricing budgets for the controlled
    path. Keep the shared estimator Rust-owned and the
    standalone Asian pricing convenience plain. Reconcile
    existing product configuration, wire, host dispatch,
    tests and examples through the dated owner. Retain product
    ownership of settlement, rate and volatility source,
    risk repricing and reported product measures.
    [Dated Asian product](../valuations-fx-and-exotics/asset-asian.md).

12. **Qualify the financial and statistical results.** Verify
    disjoint replay, sample and physical work counts, grouped
    antithetic fits, constant and tiny varying controls,
    signed and extreme arithmetic, wrong-anchor bias and
    adjusted uncertainty on independent examples. Compare
    the geometric control expectation and joint target/control
    moments with independent seasoned GBM benchmarks. Use
    repeated pilots and pricing samples to assess error and
    efficiency over intended payoff domains. Check actual
    product and host behavior independently.

## Observed behavior and limitations

The [aggregate helper](../../../../finstack-quant/models/src/monte_carlo/control_variate.rs)
uses precomputed means, sample variances and covariance. It
uses a fitted coefficient only above a control variance of
`1e-10`, floors negative adjusted variance at zero, reports
a normal 1.96 interval, and returns NaN uncertainty below
two samples. It does not establish covariance admissibility.

The [dated arithmetic Asian GBM pricer](../../../../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs)
uses it in production. It streams same-path arithmetic and
geometric payoff moments, computes a seasoned analytic control
mean, fits on the pricing sample and returns the adjusted
Money mean. Its [paired pricing route](../../../../finstack-quant/models/src/monte_carlo/pricer/path_dependent.rs)
rejects antithetic and capture requests and can pool individual
Sobol point moments. The plain standalone Asian convenience
does not select the helper.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| `C` uniform on `[0,1]`, `X=C^2`, `mu_C=1/2`, two draws | The same-sample fitted slope is `C1+C2` and gives expected adjusted mean `1/4` for a true target of `1/3`. Independent pricing with a frozen coefficient gives conditional mean `1/3`. |
| Same population, fixed `beta=1` or `3` | The raw variance is `4/45`. The adjusted variances are `1/180` and `61/180`, respectively. |
| Antithetic target/control leg pairs `((0,0),(4,2))` and `((2,4),(6,6))` | The group observations are `(2,1),(4,5)` and give `beta=1/2`. Pooling four legs gives `4/5`. At `mu_C=3`, the correct fixed adjustment gives `(3,3)`; scaling both control and its mean by `2^-30` changes beta to `2^29` and leaves the adjustment unchanged. |
| `X=(1,3), C=(2,2), mu_C=1` | The pilot cannot identify beta. With a previously frozen `beta=1`, a pricing sample with those values gives `Z=(0,2)`. |
| Supplied variances both `1` and covariance `nextUp(1)` | The implied covariance matrix is invalid. The old floor can report zero uncertainty. Checked paired input does not admit caller-supplied aggregate moments. |
| Seasoned unit GBM call with `S0=K=100, r=.04, q=.015, sigma=.30`, payment at `1.3`, history `90`, origin level `110`, two rows at `.25` with multipliers `.8,1.2` and a row at `1` with multiplier `1.05` | Independent Gaussian integration gives geometric control PV about `3.88404801`, target PV `4.92955032`, and ideal unpaired population beta `1.19637821`. Discounting the anchor to `1` instead of payment gives `3.93093736` and shifts the adjusted target by about `0.05609740` at that beta. |

The examples check the selected conditional target and
the ideal GBM control identity. Existing algebraic and
paired-path tests were inspected. Runtime, product and host
qualification remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#monte-carlo-models).
