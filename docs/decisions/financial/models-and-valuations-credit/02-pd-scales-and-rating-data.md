# MODELS-AND-VALUATIONS-CREDIT-02: PD cycle adjustment, master scales and rating calibration data

- Decision status: **Accepted — full package on 2026-11-11**.
- Implementation status: Existing Gaussian PD transforms, embedded scales, rating-factor tables and product callers observed. Checked calculation, explicit snapshots and caller reconciliation are pending.
- Validation status: Primary references, selected tests and independent conditional-PD, cohort, bin and factor examples reviewed. Runtime, host, empirical and regulatory qualification are pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked PD transforms and explicit rating data

1. **Use one Gaussian conditioning law.** The standardized
   asset variable is `sqrt(rho)*Z+sqrt(1-rho)*epsilon` with
   independent standard-normal systematic and idiosyncratic
   factors. For a qualified one-year unconditional TTC
   probability `p` and supplied realization `z`, use
   `PIT=Phi((Phi_inv(p)-sqrt(rho)*z)/sqrt(1-rho))`.
   Invert a conditional probability `q` using
   `TTC=Phi(sqrt(1-rho)*Phi_inv(q)+sqrt(rho)*z)`.
   Keep the same horizon, event and probability measure.
   The caller owns the factor, correlation and empirical
   applicability. The transform performs no horizon scaling
   or probability-measure conversion.
   [Vasicek](../../../REFERENCES.md#vasicek-2002-loan-portfolio).

2. **Interpret the factor and unconditional mean literally.**
   For positive correlation, a larger factor realization
   lowers PIT at fixed TTC. Integration over the entire
   standard-normal factor recovers TTC under the stated
   law. `z=0` is the median conditional probability and
   generally differs from TTC. An adverse factor relative
   to zero can still give PIT below TTC. For an interior
   TTC probability, equality occurs at
   `z=Phi_inv(p)*sqrt(rho)/(1+sqrt(1-rho))` when `rho>0`.
   Remove the existing average-condition equality claim.

3. **Check domains, identities and returned tails.** Admit
   probability fractions in `[0,1]`, correlation in
   `[0,1)` and finite `z`. Validate all inputs before any
   identity. Literal probability endpoints stay fixed,
   and zero correlation returns the input exactly. Unit
   correlation is singular for this two-way operation.
   For interior inputs reuse the checked standard-normal
   functions and scale-aware arithmetic. Require an
   interior returned probability. Unresolved arithmetic
   or rounding to an endpoint is a contextual computation
   error. Do not clip, nudge, floor or promise exact
   round trips.
   [Accepted distribution functions](../core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains).

4. **Keep the equal-year central tendency.** Use the checked
   arithmetic mean of a nonempty sequence of comparable
   observed one-year cohort default rates in `[0,1]).
   Give one weight to each included year. A defined
   zero-default rate is a literal observation. The owner
   qualifies the cohort denominator, event definition,
   annual window, overlap and missing or undefined years.
   No gap fill, exposure pooling or inferred weights.
   Preserve exact endpoint means and report loss of a
   strict interior mean to endpoint rounding as a range
   error. Population and cycle representativeness and
   statistical calibration remain separate.
   [Accepted checked means](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty).

5. **Retire the generic regulatory floor.** Remove the
   `0.0003` Basel-named helper and constant and their host
   exports. A regulated calculation owns its explicitly
   qualified jurisdiction, rule vintage, exposure scope,
   horizon and purpose. Preserve the unfloored model or
   observed PD at this boundary. The historical floor is
   evidence of a specified rule and vintage.
   [Historical Basel source](../../../REFERENCES.md#basel-ii-2006).

6. **Use one complete ordered PD partition.** Require a
   nonempty master scale with finite, strictly increasing
   upper bounds in `(0,1]`, ending exactly at 1. The
   first band is `[0,u0]` and later bands are
   `(u_previous,u]`. Map a finite PD in `[0,1]` to
   the first satisfied inclusive upper bound, with equality
   in the better band. Require each supplied central PD
   to classify to its own band. This also orders the
   representatives strictly. Allow 0 or 1 as a representative
   when explicitly supplied in its own band. Return the
   input PD, row label/index, supplied representative and
   selected scale identity. No interpolation, repair,
   rounding tolerance or last-row fallback. Batch mapping
   preserves input order and fails as a whole on invalid PD.

7. **Treat grade labels and mapped inputs as scale-local.**
   Normalize IDs by outer trim and ASCII lowercase.
   Trim display labels, preserve their spelling and reject
   blank or normalized duplicate IDs and labels. Reserve
   unrated status labels outside the numeric bands.
   A scale row can use its own declared label and remains
   distinct from a canonical rating symbol or assigned
   agency opinion. Require an explicit scale and a
   qualified PD whose horizon, event, population and
   measure/conditioning basis match the scale's stated
   applicability. Mapping PD 1 into the last band assigns
   that scale label and supplies no evidence of an actual
   default event.
   [Accepted ratings](../core-conventions/019-credit-ratings.md#accepted-decision-generic-ratings-and-explicit-scorecard-scales).

8. **Resolve immutable PD scale and factor snapshots explicitly.**
   Use supplied validated data, directly or through an
   explicitly selected ID in a supplied registry. Retain
   the exact ordered rows, ID, version, source, effective
   date or dated coverage and declared applicability.
   For scales retain the probability basis. For factors
   retain the methodology, units, source symbols and any
   explicit mapping to the canonical ladder. Remove
   automatic defaults and embedded assumptions as live
   calibration. Preserve their audit provenance. Require
   checked atomic Rust construction and restoration,
   including all hosts, with unknown fields, missing
   references and inconsistent results rejected. Keep
   the selected snapshot for replay. A data refresh
   requires an explicit new selection.

9. **Use exact lookup on the chosen factor table.** Require
   nonempty unique canonical non-default rated rows in
   quality order and finite nonnegative dimensionless
   factors, nondecreasing as quality weakens. Allow equal
   factors for distinct grades and gaps in coverage.
   A missing grade returns a specific missing-row error.
   `NR`, missing ratings and default status require
   product-owned treatment. Retire the implicit Moody's
   singleton, default-factor field and convenience, and
   bundled generic D and NR proxies. The historical
   May 2024 CLO exhibit specifies ten-year idealized
   default rates multiplied by 10,000 for its defined
   rating basis. It supports the historical rows through
   Ca/C and does not establish the asset's declared
   September vintage or its D, NR and fallback entries.
   Any selected agency table requires separate exact
   source, date, rating-basis and applicability evidence.
   [Dated CLO source](../../../REFERENCES.md#historical-clo-rating-factors).

10. **Invert WARF only through a selected factor table.**
    Admit finite WARF within the selected table's observed
    factor range. Choose the closest distinct factor,
    comparing exact distances between the represented
    inputs before rounding. An exact midpoint selects the
    larger factor. If more than one grade has the selected
    factor, return an ambiguity error. Do not infer a
    unique grade from row order. Remove factor lookup
    and inversion from migration `RatingScale`. Rating
    state ordering, absorbing states and migration
    dynamics retain their owner. A returned equivalent
    identifies a row in the selected factor table.
    [Migration record](03-rating-migration.md).

11. **Reconcile the live credit consumers.** Remove
    default master-scale grades from the distress score
    and Merton reports and retire the `map_score`
    shortcut. Native historical score probabilities and
    structural probabilities retain their declared
    measure, horizon and provenance. Structured-credit
    WARF requires the same explicitly selected snapshot
    across pool reports, metrics and simulation state
    when WARF is requested. Those owners determine
    balances, defaulted exposures, missing ratings and
    availability. Optional diagnostics without a selected
    table stay unavailable. Keep unaffected recovery
    and LGD policy with its owning record.
    [Accepted distress scores](01-financial-distress-scores.md#accepted-decision-for-checked-financial-distress-scores),
    [structural credit](04-structural-default.md),
    [pool statistics](../valuations-structured-credit/02-pool-normalization-and-statistics.md).

12. **Reconcile hosts and qualify each use.** Keep
    calculation, data validation and mapping in Rust.
    Reconcile existing Python/WASM APIs, result and wire
    state, generated contracts, parity, examples and
    Merton and scorecard views. Check conditional means
    and tails independently, compare equal-year and
    pooled rates, and exercise bin boundaries, missing
    rows, factor ties, mismatched applicability and
    snapshot restoration. Verify exact source and
    vintage separately from numeric round trips.
    Empirical PD calibration and regulatory or agency
    application require their own evidence.

## Observed behavior and limitations

[Cycle calibration](../../../../finstack-quant/models/src/credit/pd/calibration.rs)
implements the Gaussian formulas, rejects zero correlation
and probability endpoints, describes zero factor as equality,
and can return rounded endpoints. It also implements a raw
annual mean and the separate historical floor helper.
[Master-scale construction](../../../../finstack-quant/models/src/credit/pd/master_scale.rs)
permits incomplete coverage and central PDs outside their
own bins, and mapping falls through to the final grade.

The [embedded registry](../../../../finstack-quant/models/src/credit/registry.rs)
selects default scale and factor IDs and drops source
metadata when constructing their lookup objects. The
[assumptions file](../../../../finstack-quant/models/data/credit/credit_assumptions.v1.json)
labels PD rows as library assumptions and gives the
factor table a September 2024 vintage.
[Rating-factor lookup](../../../../finstack-quant/models/src/credit/rating_factors.rs)
preserves exact missing-row errors but exposes a default
factor accessor.
[Migration scale inversion](../../../../finstack-quant/models/src/credit/migration/scale.rs)
implicitly uses the Moody's table, skips unavailable rows
and applies an epsilon tie rule.

The [Merton report](../../../../finstack-quant/models/src/credit/merton/report.rs)
applies one default scale to probabilities at different
horizons. The [pool WARF caller](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/types/pool.rs)
substitutes NR for missing ratings.

## Independent review evidence

| Example | Expected outcome under the proposed contract |
| --- | --- |
| TTC `0.02`, `rho=0.2`, `z=0,-0.2,-1.5` | PIT is approximately `0.0108333363,0.0140402078,0.0610328344`. Equality is near `z=-0.48482435`. Independent integration over the factor gives `0.02`. |
| Literal PD endpoints or zero correlation, with otherwise valid input | Return the declared identity. Unit correlation fails. Interior output rounded to an endpoint at an extreme finite factor fails the requested transform. |
| Annual cohorts with 1 default among 100 and 90 among 900 | Equal-year mean is `0.055`. Pooling obligors would give `0.091`. |
| Scale uppers `0.02,1`, representatives `0.01,0.10` | PD exactly `0.02` maps to the first band. Its next higher binary64 value maps to the second. The second representative equal to `0.02` fails construction, as does a last upper below 1. |
| Factors `360,610` | Exact midpoint `485` selects 610. The next lower represented WARF selects 360. An input outside the factor range fails. |
| Factors `2^-60,1` and WARF `0.5` | The exact distances select `2^-60`. Rounded subtraction alone can produce a false tie. |
| Historical table rows Ca and C both at `10000` | Forward lookups can return that factor. Selecting it in the inverse yields an ambiguity error. |

[Existing tests](../../../../finstack-quant/models/src/credit/pd/tests.rs)
cover cycle round trips and scale mapping. The independent
examples check the proposed calculations and data invariants.
Runtime, host, empirical and regulatory qualification remain pending.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
