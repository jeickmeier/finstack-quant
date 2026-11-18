# MARGIN-COLLATERAL-IM-018 — Signed variation margin, MTA and rounding

**Decision status:** Accepted 2026-11-17 for checked signed variation margin, MTA and rounding.

**Implementation status:** Existing behavior is described below. Agreement-qualified inputs, side thresholds, separate independent amounts, checked transfers and caller reconciliation with the accepted contract are pending.

**Validation status:** Primary margin and CSA references inspected. Independent Decimal examples checked signed thresholds, MTA equality, post-gate rounding, side reversal and an elected full return. Runtime, replay and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for checked signed variation margin and transfers

1. Require qualified signed exposure `E` and credited VM balance `C` for the same agreement, authorized account and cut in the agreement currency. Positive exposure means the counterparty owes the desk. Positive `C` means collateral held and negative `C` means collateral posted. Use the elected treatment of eligible pending transfers. Require explicit zero for an empty balance. [Collateral valuation](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value) · [Portfolio VM scope](../portfolio-execution/17-margin-collateral-and-wire.md#accepted-decision-for-checked-portfolio-vm-and-collateral-reports).
2. Require explicit nonnegative collection and posting thresholds `Tc` and `Tp`. Calculate signed required VM as `R = max(E − Tc, 0) − max(−E − Tp, 0)`. Elect zero thresholds where the applicable regime requires full VM. Remove implicit threshold and rounding defaults. Keep independent amounts and IM separate. An agreement that combines independent amounts with VM requires its own qualified calculation. [Basel MGN20.4](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn/20/inforce/2019-12-15/published/2019-12-15) · [ISDA separate margin flows](https://www.isda.org/a/x0iDE/form-of-amendment-to-the-1994-isda-csa-ny-law.pdf).
3. Calculate `U = R − C` in checked Money. A positive difference moves cash or collateral to the desk. A negative difference means the desk delivers or returns collateral. Net desk outflow is the negative of the final signed transfer. [Money arithmetic](../core-conventions/012-money-decimal-precision.md).
4. Compare the absolute unrounded difference to the applicable direction's elected MTA once, using the agreement's at-least or strictly-greater-than boundary. A shared IM and VM MTA requires a qualified joint decision. An exact zero difference produces no transfer. [IM account terms](17-csa-selection-and-im-terms.md#accepted-decision-for-checked-csa-elections-and-one-way-im-account-terms) · [EU Article 25](https://eur-lex.europa.eu/legal-content/EN/TXT/PDF/?uri=CELEX%3A02016R2251-20230214) · [CFTC combined MTA rule](https://www.ecfr.gov/current/title-17/chapter-I/part-23/subpart-E/section-23.152).
5. After the MTA gate, use either explicit no rounding or the elected positive increment with delivery rounded up and return rounded down in magnitude. A nonzero difference in the direction of a nonzero required balance is delivery, including an authorized single net transfer across collateral sides. Other differences are returns. Honor an expressly elected full-return exception when the target is zero. Preserve the original MTA decision after rounding. [Example of negotiated rounding terms](https://www.sec.gov/Archives/edgar/data/1397612/000127727707000407/exh43to8kpsawamu07_he3.pdf).
6. Use one checked Rust calculation and validation through portfolio, replay, Python and WASM. Report raw exposure, required VM, credited balance, unrounded difference, final transfer and reason for any zero. Distinguish MTA suppression, rounding to zero and unavailable inputs. Record 019 owns call timing and settlement state. Record 022 owns the wire format. [Call and settlement timing](19-margin-dates-and-replay.md#accepted-decision-for-checked-vm-schedules-and-settlement-replay) · [Result transport](22-wire-tables-and-provenance.md).

A USD 151,001 return passes an elected USD 151,000 MTA and rounds down to USD 150,000 on a USD 10,000 increment. A positive signed transfer of USD 20 gives a net desk outflow of negative USD 20.

## Choice and rationale

The observed VM calculation uses one signed balance, a symmetric threshold and one additive independent amount. The accepted calculation uses the elected threshold for each side and preserves the signed target and transfer evidence.

## Input domain and conventions

Exposure>0 means counterparty owes desk. Collateral>0 is held; collateral<0 is posted, including pending agreed calls. All amounts are in CSA base currency; threshold, MTA, rounding and independent amount must be finite/nonnegative. This model has one additive nonnegative IA, not separate independent amounts by posting party.

## Ordered algorithm

1. Compute threshold-adjusted support `R=sign(E)*max(abs(E)-T,0)+IA`, returning positive zero for the zero excess before adding IA. Nonfinite exposure and invalid elections error.
2. Unrounded signed call U=R-C. Check collateral currency. If abs(U)<MTA return zero; equality proceeds.
3. If rounding increment q>0 and U≠0, round abs(U)/q upward when U×R>0 (delivery toward required support), otherwise downward (return); restore U's sign. q=0 disables rounding. MTA is not rechecked after rounding.
4. VmCalculator splits collect=max(call,0), post=max(-call,0), while reporting R as net_exposure and storing raw U, prior C, threshold, IA, MTA and q.
5. `net_margin` is desk cash outflow: positive post, negative collect, positive zero for no transfer. Calls use positive transfer magnitudes and call types for direction.

## Selection, controls and limitations

`VmParameters::with_threshold` chooses daily/T+1/round 10,000/IA 0; registry defaults are separate. VmCalculator validates the CSA and both currencies on every call; no FX, haircut valuation, interest accrual, model IM or portfolio netting is performed by this stage.

## Alternatives and tradeoffs

The accepted transfer gate uses the unrounded difference. Preserve its decision when delivery or return rounding changes the transfer magnitude. Independent amounts belong to their elected collateral operation.

## Source and validation evidence

Inspected assertions include raw 147k below 150k MTA giving zero despite possible rounding; delivery 151001 rounded to 160000, return 151001 rounded to 150000; and negative exposure with overposted collateral returning 155k rounded down to 150k. VM result replay and cashflow-direction regressions are invariant/formula evidence, not independent CSA legal review.

Implementation and data: [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [src/types/call.rs](../../../../finstack-quant/margin/src/types/call.rs).

Existing evidence: [src/types/thresholds.rs](../../../../finstack-quant/margin/src/types/thresholds.rs), [src/calculators/vm.rs](../../../../finstack-quant/margin/src/calculators/vm.rs), [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
