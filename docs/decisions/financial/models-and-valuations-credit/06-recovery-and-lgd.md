# MODELS-AND-VALUATIONS-CREDIT-06: Recovery dilution, LGD, EAD and priority allocation

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Recovery combines separately selectable deterministic dilution, collateral workout, stressed LGD, beta-distributed seniority recovery and an explicit priority waterfall. These are complementary calculators, not one automatically calibrated framework.

## Scope and input domain

Recovery/haircuts/CCF in [0,1], nonnegative collateral/exposures/costs, positive EAD for workout. Beta mean is strictly inside (0,1) with positive variance below mean×(1−mean). Estate includes pledged collateral.

## Algorithm

1. Dynamic recovery is unchanged below base notional; above it use constant recovery, inverse notional, inverse power (exponent in (0,1]), floored inverse, or floored linear decline in notional ratio. Nonpositive current notional returns zero and output never exceeds base recovery.
2. EAD is drawn+CCF×undrawn; revolver helper fixes CCF=.75 and term loans use full draw. Observed LEQ is (observedEAD−drawn)/undrawn with no clipping, absent when undrawn≤0; zero commitment utilization is zero.
3. Workout gross recovery is min(sum collateral×(1−haircut),EAD). Subtract direct+indirect cost rates times EAD, discount both by `(1+r)^−years`, floor net recovery at zero, and clamp LGD=1−net/EAD.
4. Downturn stress approximation adds sensitivity×√correlation×Φ⁻¹(quantile)×√[LGD(1−LGD)]; regulatory-floor mode takes max(base+add_on,floor), then clamps [0,1]. Registry supplies default workout/downturn/seniority calibrations.
5. Beta parameters use common=m(1−m)/variance−1, alpha=m×common, beta=(1−m)×common. Sampling delegates the shared beta sampler (seeded PCG64 helper); quantiles delegate statrs Beta inverse CDF.
6. Waterfall validates unique trimmed claim IDs, pays haircut collateral up to each claim, subtracts it from estate, sorts ascending numeric priority, and pays residual deficiency pro rata within each class. Compensated sums and a 64-epsilon relative tolerance govern conservation; tiny excess is removed from final allocations. APR flag is true after this constructed priority ordering.

## Selection and controls

Caller selects calculator/variant and registry IDs; builders use [embedded defaults](../../../../finstack-quant/models/data/credit/credit_assumptions.v1.json): approximate Moody’s 1982–2023 seniority moments, `basel_secured` add-on .08/floor .10, and two-year workout with 5% discount, 5% direct and 3% indirect costs. Recovery table IDs are exact; listing places the configured default first. Numeric priority, not the descriptive seniority string, controls waterfall order. No legal ranking engine is invoked.

## Conventions

All money inputs must share an external currency/unit because these calculators use f64. Costs are fractions of EAD; workout discounting is annual compounding. CCF=.75 is a named preset, not a universal present regulatory requirement.

## Limitations, failure and fallback

Base recovery does not rise after deleveraging. Stressed quantiles below .5 can reduce LGD. Public workout component fields can be mutated after construction; the final calculator rebuild validates them. The waterfall assumes distinct collateral pools and a valid supplied priority structure; APR=true is not a legal opinion.

## Alternatives and tradeoffs

Multi-period recovery timing, endogenous collateral prices, alternative downturn distributions and legally contingent ranking are unimplemented. Full-draw and user-supplied CCF are supported alternatives to the revolver preset.

## Evidence

Owners: [dynamic_recovery.rs](../../../../finstack-quant/models/src/credit/dynamic_recovery.rs), [LGD implementations](../../../../finstack-quant/models/src/credit/lgd), [recovery_waterfall.rs](../../../../finstack-quant/models/src/credit/recovery_waterfall.rs), [registry.rs](../../../../finstack-quant/models/src/credit/registry.rs). [recovery_waterfall.rs tests](../../../../finstack-quant/models/tests/recovery_waterfall.rs) check payout conservation; inline LGD tests check domains, beta moments and formulas; none establishes empirical default-loss calibration. References: [LGD](../../../REFERENCES.md#schuermann-2004-lgd), [Recovery dependence](../../../REFERENCES.md#altman-et-al-2005-recovery).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
