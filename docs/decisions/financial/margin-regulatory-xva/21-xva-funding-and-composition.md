# MARGIN-REGULATORY-XVA-021: Asymmetric FVA, bilateral activation and total-XVA sign contract

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

Funding cost applies to positive expected exposure, benefit to negative exposure. Configuring an IM profile also changes bilateral DVA and activates MVA. Optional-leg presence therefore has financial meaning even when a spread is zero. The observed default FundingConfig has zero cost spread and symmetric zero benefit.

## Algorithm and conventions

1. Validate finite nonnegative cost spread, optional benefit and optional IM spread in basis points. Explicit benefit must not exceed cost; absent benefit defaults to cost. Absent margin spread defaults to cost, but an explicit margin spread may exceed cost. Validate optional IM profile.
2. Standalone FVA interval cost is `(avg(EPE)×cost_bp/10000 − avg(ENE)×benefit_bp/10000)×avg(DF)×dt`. There is no survival factor. Negative total FVA is a funding benefit.
3. Bilateral FVA multiplies that interval by `avg(S_cp)×avg(S_own)`. This is a product of averages, not the average of the joint survival product. Posted IM does not reduce the EPE/ENE used for FVA.
4. Bilateral orchestration validates funding, computes bilateral CVA, verifies any IM terminal time matches exposure terminal time within `1e−12×max(abs(end_times),1)`, and computes DVA with IM reduction where supplied. The intermediate IM dates need not match exposure dates.
5. Funding config absent means FVA/MVA absent but DVA still computed. Funding config present computes FVA even when zero; IM present computes MVA with its effective flat spread and both survival curves. IM still reduces DVA when its funding spread is zero.
6. `total_xva=CVA−DVA+FVA+MVA`, treating uncomputed optional legs as zero. It is intended to be subtracted from risk-free value. Results retain original exposure profiles, not the IM-reduced DVA exposure; CVA/DVA/FVA rows replay the individual kernels, but XvaResult does not include MVA bucket rows.

## Selection and controls

The standalone FVA helper validates the entire FundingConfig, including IM even though that helper does not price MVA or change the raw exposure profile. Public spread fallback accessors return values without revalidating; computation entry points validate first. The bilateral wrapper creates a one-point flat IM spread curve at zero; standalone MVA can accept a term structure.

## Limitations and failure behavior

Positive DVA is a benefit under the explicit composition and DVA field. Funding survival weighting does not by itself resolve possible CVA/DVA/FVA double counting or own-credit/funding transfer-pricing choices. Netting/collateral perimeter and currency alignment are external inputs. No parameter calibration, funding-spread sourcing or stochastic funding model is supplied.

## Alternatives and tradeoffs

A unified funding/credit valuation equation can address feedback and double counting at greater modelling cost. Separate cost/benefit curves would capture term structure, whereas the current FVA interface uses flat spreads. Removing absent-vs-zero distinctions would lose whether a leg was actually requested.

## Evidence and role

Config/accessors/result composition: [src/xva/types.rs](../../../../finstack-quant/margin/src/xva/types.rs); routing and FVA: [src/xva/cva.rs](../../../../finstack-quant/margin/src/xva/cva.rs). Inspected flat-funding test compares against a discounted flat-exposure closed form at 3% tolerance; inspected bilateral tests establish survival-weighted inequalities. Located tests cover asymmetric benefit, IM horizon mismatch, posted-IM DVA and zero-spread activation. These do not establish funding economics or independent joint-default validation. [Gregory XVA](../../../REFERENCES.md#gregory-xva-challenge), [Green XVA](../../../REFERENCES.md#green-xva).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
