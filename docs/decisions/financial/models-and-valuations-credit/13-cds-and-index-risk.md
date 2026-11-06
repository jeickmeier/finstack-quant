# MODELS-AND-VALUATIONS-CREDIT-13: CDS and index quote risk, default exposure and margin proxy

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Quote-space CS01/rebootstrap is distinct from coupon risky PV01 and direct hazard risk. Rate/recovery calculations conditionally replay calibration metadata; margin can fall back to a duration proxy.

## Scope and input domain

Single-name CDS and CDS indices, with active constituent hazard curves when configured. CS01 requires quote replay and a provider; missing rate calibration permits direct discount-curve bumps.

## Algorithm

CS01 rebuilds spread quotes and centrally differences raw reprices per bp; bucketed CDS uses cached schedule/discount factors. CS gamma uses up/down/replayed center divided by decimal spread bump squared. DV01 bumps rate calibration quotes when metadata exists, otherwise direct curve rates, then replays available hazard recipes under changed discount market. Index deduplicates active hazards. Recovery01 changes contractual recovery and rebuilds matching hazard; index applies active constituent recoveries and normalizes by weighted actual boundary span.
Expected loss=N×conditional default probability to maturity×LGD, unsigned and undiscounted; index weights active names and surviving factor. CDS JTD=signed LGD payout minus/plus accrued coupon; LGD-only excludes accrued, default exposure subtracts current PV. CDS protection-start/maturity gates apply to payout. Index JTD averages single-name LGD, uses active weights/factor for constituents, or original notional/name count for single curve; named count guesses warn and unknown names fail.
SIMM tries repriced CS01, otherwise warns and returns signed |N|×years×LGD×duration factor×1bp. Nonpositive maturity time substitutes standard CDS maturity before tenor bucketing. Index proxy scales notional by index factor.

## Selection and controls

Resolved sensitivity config controls quote/rate bumps. Doc-clause family and clean quote override feed single-name replay. Index JTD count can be explicit or inferred from name (125 IG/iTraxx,100 HY,75 crossover,40 EM).

## Conventions

CS01/DV01 per bp; CS gamma per decimal-spread squared. JTD and default exposure are different payoff/MTM definitions. Expected loss ignores protection-effective start, so it can include pre-protection default probability for forward starts.

## Limitations, failure and fallback

Rate/recovery frozen-versus-replayed behavior depends on metadata. Index JTD has no as-of maturity check; margin fallback may assign positive five-year-like risk to expired instruments if repricing fails. Quote risk fallback is not equivalent to calibrated market risk.

## Alternatives and tradeoffs

Fail-closed missing risk and explicit frozen/calibrated risk names are alternatives; coupon risky annuity is already a separate supported metric.

## Evidence

Owners: [CDS metrics](../../../../finstack-quant/valuations/credit/src/cds/metrics), [index metrics](../../../../finstack-quant/valuations/credit/src/cds_index/metrics), [cs01.rs](../../../../finstack-quant/valuations/credit/src/cs01.rs), [marginable.rs](../../../../finstack-quant/valuations/credit/src/cds/marginable.rs). Tests under [credit tests](../../../../finstack-quant/valuations/credit/tests) locate reprice, convention and factor checks; no execution performed.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
