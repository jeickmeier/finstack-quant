# STATEMENTS-ANALYTICS-CREDIT-15: Stage-specific PD/LGD/EAD integration and impaired recovery

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Performing exposures sum unconditional default mass times effective LGD, midpoint EAD and EIR discounting. Stage 3 instead measures current exposure less discounted expected recovery. The distinct impaired formula preserves recovery delay's time-value loss even when undiscounted LGD is zero.

## Scope and input domain

Validated Exposure, caller-supplied stage, PD source and `EclConfig`. Bucket width must be finite and >= .0001 years. Stage-3 recovery delay is finite/nonnegative. Config validation also validates staging settings, even though this function does not classify the stage.

## Algorithm

1. Select effective LGD: PointInTime uses exposure LGD and rejects TTC/downturn settings; ThroughTheCycle requires an explicit finite [0,1] `ttc_lgd` and no downturn adjuster; Downturn requires a valid models `DownturnLgd` adjuster and no TTC field, then applies that adjuster to base LGD.
2. Stage 1 sets horizon min(1 year, remaining maturity); Stage 2 uses remaining maturity. Use `ceil(horizon/bucket_width)` buckets, with at least one even for zero maturity. Final endpoint clips to horizon; midpoint is the average of endpoints.
3. At each bucket, look up cumulative PD at both endpoints using current rating or literal `NR`. Require finite probabilities in [0,1] and nondecreasing within the interval. Use unconditional mass `F(end)−F(start)`, not conditional marginal PD without survival weighting.
4. Compute EAD at midpoint and `DF=(1+eir)^−midpoint`; bucket allowance is default mass × effective LGD × EAD × DF. Accumulate ordinary floating-point addition and store every bucket's terms. Reject non-finite final allowance.
5. Stage 3 still validates a PD-source lookup at t=0 for current rating/NR, though future default probability is not used. Set EAD at zero and allowance `EAD0−(1−effective LGD)*EAD0*(1+eir)^−recovery_delay`. Return one bucket with marginal PD=1 and reported horizon equal to recovery delay, independent of remaining maturity; zero contractual maturity can still have an allowance.

## Selection and controls

The caller supplies stage explicitly; staging and measurement are separable APIs. Default width is .25 years and impaired recovery delay one year. LGD selection is explicit and inconsistent combinations error. The downturn algorithm itself is owned by models credit [recovery/LGD](../models-and-valuations-credit/06-recovery-and-lgd.md).

## Conventions

PD mass is unconditional from the reporting-date curve; bucket `marginal_pd` stores that unconditional increment. Annual effective EIR discounts with fractional years. Stage 1 restricts the default-event horizon, while LGD is the supplied loss fraction rather than an explicit post-default recovery schedule. Output has the caller-supplied EAD unit; currency consistency is external and not validated because Exposure and EclResult contain scalar amounts without a Currency/Money field. Results use default `ResultsMeta`.

## Limitations, failure and fallback

The grid does not split at PD knots or EAD schedule changes; midpoint EAD/discount is approximate. The final finite guard is not an accuracy bound. No compensation is used in summation. Exposure currency and resolved model/policy choice are not carried in a dedicated result field. Stage-3 source membership is required even though its PD is economically irrelevant. No multiple recoveries, cure cashflows or discounted cash-shortfall schedule is modeled.

## Alternatives and tradeoffs

Adaptive or event-aligned integration can reduce bucket error but needs convergence controls. Direct expected cash-shortfall discounting supports richer recovery timing at a higher data cost. A stage-3 measurement without an unused PD dependency would simplify the contract but changes validation behavior.

## Evidence

[engine.rs](../../../../finstack-quant/statements-analytics/src/analysis/ecl/engine.rs) owns calculation. Inspected `test_ecl_hand_computed` independently computes two 0.01 default masses × .4 LGD × 100000 at zero EIR = 800; this isolates unconditional versus conditional PD. Located tests cover Stage-3 carrying-value-less-PV-recovery, zero maturity, recovery delay, EAD schedules and TTC/downturn selection. [IFRS 9 reference](../../../REFERENCES.md#ifrs-9-impairment) is framework context, not proof of complete accounting implementation.

The accepted [numerical accumulation policy](../../standards/0042-numerical-accumulation-and-comparisons.md) remains applicable to allowance and scenario sums; this record does not reopen it.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
