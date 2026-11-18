# MARGIN-REGULATORY-XVA-014: SA-CCR trade domain, supervisory delta and netting-set validation

- Decision status: **Accepted 2026-11-17 for checked SA-CCR trade inputs and supervisory delta.**
- Implementation status: Existing behavior is described below. Qualified trade and set assembly, supervisory-delta calculation, and validated result and host contracts remain pending.
- Validation status: Primary references and Rust input paths checked. Independent arithmetic covered option signs and magnitudes, tranche scale, maturity versus exercise, FX leg selection and example MPOR floors. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked SA-CCR trade inputs and supervisory delta

1. **Bind the calculation to an effective rulebook and legal set.** Require
   the jurisdiction and versioned regulatory profile, valuation cut, reporting
   currency, counterparty, clearing role, trade IDs and coverage evidence.
   Use signed bank-perspective MTM on the same cut with dated FX evidence.
   Group trades only under documented eligible closeout netting. Treat
   transactions without eligible netting as individual sets. Use alpha
   `1.4` for ordinary Basel SA-CCR and remove the public scalar override.
   The elected regulatory profile owns its parameters.
   [Basel CRE52.1 and 52.7–8](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
2. **Derive margin treatment and MPOR from the agreement and exposure.**
   Treat a set as margined when the counterparty must post variation margin
   (VM). A bank-only posting agreement uses unmargined treatment and retains
   posted VM with a negative sign. Use explicit signed VM and net independent
   collateral amount (NICA) and finite nonnegative threshold and minimum
   transfer amount where applicable. Use the higher of the estimated
   business-day MPOR and the applicable floor. The Basel bilateral floor
   is ten business days for daily margining and nine plus the remargining
   interval otherwise. Apply the 20-business-day minimum under CRE52.51
   for non-CCP sets with more than 5,000 transactions or sets with eligible
   illiquidity triggers, including illiquid collateral or an OTC derivative
   that cannot easily be replaced in stress. If more than two qualifying
   disputes in the prior two quarters each exceeded the MPOR before the dispute
   adjustment, double the applicable floor for the next two quarters. Count
   only VM disputes where MGN20 applies. Apply the five-day allowance only
   to a qualifying clearing exposure, such as a clearing member's exposure
   to its client on a cleared transaction under CRE54.12.
   [Record 018](18-saccr-rc-pfe-ead.md) owns the assembly for mixed margin
   agreements and collateral shared across legal sets.
   [Basel CRE52.2, 52.10, 52.50–51 and 52.74–77](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27) ·
   [Basel CRE54.8 and 54.12](https://www.bis.org/basel_framework/chapter/CRE/54.htm?inforce=20230101&published=20200327&tldate=20210706) ·
   [ADR 0044](../conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components).
3. **Validate lifecycle and distinguish the four clocks.** Base `M` on the
   latest day the trade may remain active, `T` on the latest exercise date,
   and `S` and `E` on the IR or credit reference period. A six-month
   physically settled option into a five-year swap has `T = 0.5` and
   `M = 5.5` years in the Basel example. For Bermudan swaptions, use earliest
   exercise for `S` and the underlying swap end for `E`. The next reset
   supplies maturity only when the contract settles the exposure and resets
   its fair value to zero. Resolve remaining obligations for expired,
   exercised or matured trades from the recorded lifecycle.
   [Record 015](15-saccr-maturity-and-duration.md#accepted-decision-for-checked-sa-ccr-maturity-factors-and-supervisory-duration)
   owns the year conversion, supervisory duration and maturity factor.
   [Basel CRE52.31–32, 52.37 and 52.42](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
4. **Qualify risk assignment and size.** Bind an ordinary trade to its
   primary factor, prescribed asset class and applicable category. Use
   sensitivity and volatility evidence for hybrid assignments. Link any
   supervisor-required multiple allocations to the same transaction with
   separate factor signs and deltas and count its MTM once. Enforce consistent
   categories per risk entity across hedging sets. Require a finite
   nonnegative size with its source basis. IR and credit use reporting-currency
   notional before supervisory duration, including the prescribed treatment
   for variable notionals, leverage and principal exchanges. FX uses the
   nonreporting leg or, when both legs are foreign, the larger converted leg.
   Equity and commodity size uses price times units, with the prescribed
   volatility or variance basis for volatility trades. A qualified zero size
   contributes zero add-on and retains its MTM.
   [Basel CRE52.26–37](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
5. **Compute ordinary supervisory delta in Rust.** Use exact `+1` when a
   linear trade gains as its primary factor rises and `−1` when it loses.
   For ordinary options, set `s = +1` for bought and `−1` for sold, and
   `q = +1` for call and `−1` for put. Use `δ = s × q × Φ(q × z)`, where
   `z = [ln(P/K) + 0.5σ²T] / (σ√T)` and `Φ` is the standard normal CDF.
   Require a qualified underlying price `P`, strike `K`, positive exercise
   time in years and the profile's prescribed supervisory volatility `σ`.
   Use the applicable spot or forward and the current payoff average for
   Asian options. The Basel shifted rate convention applies the same
   qualified currency-level shift to price and strike. Require both log
   inputs to be positive on the selected basis. Use checked evaluation
   and permit a representable zero from a valid tail. At one year and at the
   money, the Basel FX volatility of 15% gives about `+0.529893` for a
   bought call and `−0.470107` for a bought put.
   [Basel CRE52.39–42 and Table 2](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27) ·
   [BCBS 279, paragraphs 111–112](https://www.bis.org/publications/201403-standards-standardised-approach-measuring-counterparty-credit-risk-exposures.pdf#page=16).
6. **Require qualified paths for special payoffs.** Use
   `15 / [(1+14A)(1+14D)]` for tranches, positive for purchased protection
   and negative for sold protection. Use attachment and detachment fractions
   `0 ≤ A < D ≤ 1`. For nth-to-default on `m` names, require
   `1 ≤ n ≤ m` and use `A = (n−1)/m` and `D = n/m`. These deltas can
   exceed one in magnitude. Represent digitals with the prescribed
   `0.95K` and `1.05K` European collar sized to match the payoff outside
   the strike interval. Cap the absolute combined effective notional at the
   digital payoff magnitude divided by its supervisory factor. Treat the
   prescribed European payoff components separately and allow caplet and
   floorlet decomposition with component dates. An unsupported required
   treatment makes the calculation unavailable.
   [BCBS 279, paragraph 113](https://www.bis.org/publications/201403-standards-standardised-approach-measuring-counterparty-credit-risk-exposures.pdf#page=16) ·
   [Basel CRE52.41–43](https://www.bis.org/committees/bcbs/basel-framework/standard/cre/52/inforce/2023-01-01/published/2020-03-27).
7. **Validate the complete input and preserve its evidence.** Use typed
   terms in place of the redundant option flag, floating direction and caller
   delta. Reject blank required identifiers, duplicate transaction IDs,
   inconsistent dates or categories and unresolved required evidence.
   Same-day option inputs require a qualified exercise time or a resolved
   settlement state. Preserve profile, assignments, lifecycle, size,
   deltas and MPOR evidence through Rust, wire, Python and WASM. Records
   [016](16-saccr-ir-addon.md) and [017](17-saccr-non-ir-addons.md) own
   hedging and add-on aggregation. [Record 018](18-saccr-rc-pfe-ead.md)
   owns RC, PFE, EAD and the separately qualified premium-related EAD
   treatments.

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

SA-CCR starts from one externally assembled netting set and caller-classified trades. It consumes one consistent reporting currency, signed mark-to-market and supervisory deltas; it does not price trades, derive option deltas, perform FX conversion, or establish legal netting eligibility. The observed validation aims to prevent sign/category inconsistencies, but is not a complete lifecycle or option-input validator.

## Algorithm and conventions

1. Require finite notional, direction, supervisory delta and MTM. Direction must be nonzero, while supervisory delta lies in `[-1,1]`. Effective signed notionals use `delta×abs(notional)`: negative notionals do not reverse exposure.
2. For linear trades, `abs(abs(delta)−1)<=1e-6` and `delta×direction>0`. The direction magnitude is otherwise unused. For options require nonzero delta and an option type: CallLong/PutShort require positive delta; CallShort/PutLong require negative delta.
3. `is_option` must equal whether `option_maturity_date` is supplied, and option expiry must not exceed the underlying end date. IR/FX require no supervisory category; credit/equity/commodity require a category whose asset class agrees.
4. Across the whole supplied book, enforce the same supervisory category for the same `(asset_class,underlier)`, even when trades use different hedging-set labels.
5. Netting configuration requires finite signed collateral/NICA and finite nonnegative threshold/MTA. For margined sets MPOR must be at least 10 business days for bilateral IDs or 5 for cleared IDs. Unmargined configurations do not apply an MPOR floor. No external CCP check supplements the ID's enum tag.
6. Engine alpha defaults to `1.4`; overrides must be finite and at least one. Constructors populate fields but the calculation invokes validation. The unmargined constructor sets threshold/MTA/NICA to zero and MPOR to 10; fields may subsequently be changed.

## Selection and controls

Every trade is validated before calculation; validation is not restricted to nonzero notionals or currently live trades. `SaCcrTrade.notional` is pre-duration notional for IR/credit and already-adjusted notional for FX/equity/commodity. An option's supervisory delta magnitude must therefore be supplied under the intended market/regulatory convention. Config `as_of` controls the calendar-day offsets used by duration and maturity factors.

## Limitations and failure behavior

Option direction is checked only for finiteness/nonzero, not cross-checked with the long/short option type; option type determines the delta sign. Linear trades may carry an unused `option_type`. Blank IDs/underliers/hedging sets, duplicate trades, zero notionals, reversed underlying dates and already-matured trades are not rejected. This record does not reopen the held-by-bank positive collateral direction already recorded in [financial perspectives ADR](../conventions/0044-financial-perspectives-and-boundary-conversions.md); record 18 describes the distinct field scope.

## Alternatives and tradeoffs

The accepted boundary places supervisory-delta calculation in Rust.
Callers qualify the trade terms, primary factor and market price. The
regulatory profile supplies the applicable supervisory volatility and
margin rules. The observed path gives the caller's option type precedence
when checking the supplied delta sign.

## Evidence and role

Runtime validation and fixed category mapping: [src/regulatory/sa_ccr/types.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/types.rs); book-level category and alpha checks: [src/regulatory/sa_ccr/engine.rs](../../../../finstack-quant/margin/src/regulatory/sa_ccr/engine.rs). Netting ID variants are declared in [src/types/netting.rs](../../../../finstack-quant/margin/src/types/netting.rs). Located type/engine tests cover wrong delta signs, absent option type/expiry, category mismatch, nonfinite fields and invalid MPOR; inspected [tests/audit_regressions.rs](../../../../finstack-quant/margin/tests/audit_regressions.rs) checks MPOR rejection. These are contract tests, not independently calibrated option deltas. [SA-CCR reference index](../../../REFERENCES.md#bcbs-279-saccr).

The accepted decision records the selected methodology. Implementation and
qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
