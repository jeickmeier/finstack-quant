# MODELS-AND-VALUATIONS-CREDIT-02: PD cycle adjustment, master scales and rating calibration data

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Gaussian one-factor TTC/PIT transforms, arithmetic annual default-rate means and explicit rating lookup tables are implemented. Registry defaults select embedded data; they do not establish approval of its empirical vintage.

## Scope and input domain

TTC/PIT PD and asset correlation must be strictly between zero and one; the cycle index is finite. Central tendency accepts a nonempty sequence in [0,1]. Master-scale mapping accepts [0,1], with strictly increasing grade upper bounds.

## Algorithm

1. TTC→PIT is `Φ((Φ⁻¹(p)−√ρ z)/√(1−ρ))`; PIT→TTC applies the algebraic inverse.
2. Central tendency is the unweighted arithmetic mean of annual rates. The separately called floor helper returns `max(pd,.0003)` and preserves NaN; it does not impose an upper bound.
3. Master-scale assignment chooses the first upper bound satisfying `pd <= upper_pd`; values above every bound map to the last grade. The supplied central PD is returned, not interpolated.
4. Score mapping requires `implied_pd`; Altman results fail this requirement. Embedded registry IDs resolve scale and rating-factor data. Rating-factor lookup requires an exact supported CreditRating; the stored default factor is an accessor, not an automatic missing-rating substitute.
5. WARF inversion in RatingScale minimizes absolute distance to supported factors, breaking exact ties toward the higher factor.

## Selection and controls

Caller supplies cycle parameters or chooses registry IDs; `default_calibration` uses registry default IDs and the Moody’s helper explicitly selects `moodys_standard`. Registry parsing/validation and OnceLock caching retain errors.

## Conventions

PD is a probability fraction, cycle index a standard-normal factor, correlation a fraction, and rating factors dimensionless. Annual-rate averaging is by year, not exposure-weighted.

## Limitations, failure and fallback

Master-scale validation does not require each central PD to lie inside its bin or increase with grade, and values above the final upper bound are silently assigned to that final bin. The .0003 helper is named for a particular Basel convention; it is not a current universal regulatory floor.

## Alternatives and tradeoffs

Exposure-weighted central tendency, empirical calibration and monotone fitted scales would be separate choices; no such replacement is selected here.

## Evidence

Owners: [calibration.rs](../../../../finstack-quant/models/src/credit/pd/calibration.rs), [master_scale.rs](../../../../finstack-quant/models/src/credit/pd/master_scale.rs), [rating_factors.rs](../../../../finstack-quant/models/src/credit/rating_factors.rs), [registry.rs](../../../../finstack-quant/models/src/credit/registry.rs), [scale.rs](../../../../finstack-quant/models/src/credit/migration/scale.rs). [PD tests](../../../../finstack-quant/models/src/credit/pd/tests.rs) provides transform roundtrip/boundary evidence; roundtrips do not validate empirical calibration. References: [Vasicek](../../../REFERENCES.md#vasicek-2002-loan-portfolio) and [Basel II 2006](../../../REFERENCES.md#basel-ii-2006). The [embedded assumptions file](../../../../finstack-quant/models/data/credit/credit_assumptions.v1.json) identifies the default PD scale as library assumptions with agency-style labels, explicitly not empirical agency default rates; its rating-factor table declares a September 2024 methodology vintage.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
