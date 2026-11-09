# CASHFLOWS-009 — Fixed, periodic and utilization-tier fees

- Decision status: **Proposed — complete explicit fee-parent, amount and tier package, 2026-11-09; adoption pending**.
- Implementation status: **Observed; reconciliation pending**, scoped to fee specs/compiler/emission and their actual consumers.
- Validation status: **Production evidence inspected/located, not executed**. Independent bounded exact examples and read-only source/consumer reviews are reported below; these do not establish contractual applicability or production qualification.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` ([slice audit](README.md)).

## Choice and domain

The builder supports fixed dated amounts and periodic annual-basis-point fees on drawn or undrawn principal. Observed PointInTime is the default accrual basis; observed TimeWeightedAverage integrates economic balance changes using direct segment day counts. Tier evaluation is a standalone rate selector, used by a revolving-credit product but not automatically connected to periodic builder emission. The replacement below selects explicit fee-parent economics without adopting every product fee clause.

## Proposed decision: explicit fee parents, exact amounts and literal tier selection

Proposed on **2026-11-09** for complete whole-record review. Inherit accepted
[001's periods and complete day-count context](01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context),
[003's covered economic stock and event phases](03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases),
[005's original-parent prefix/amount pattern](05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
and [core012's exact monetary inputs and restoration](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).
This package applies that pattern to a fee's OWN parent, without treating fees as
coupons or importing cash/PIK allocation. No runtime changes are authorized here.

1. **Retain one explicit, limited periodic-fee recipe.** Keep Fixed and
   PeriodicBp. PeriodicBp requires base, exact annual bp, positive frequency,
   day_count, payment business_day_convention, calendar_id, stub and accrual_basis.
   Remove serde/host/default omission for BDC, stub and accrual basis. The selected
   PeriodicBp recipe deliberately uses Natural anchors, unadjusted accrual,
   no EOM and zero payment lag; these are disclosed operator choices, not market
   defaults or overrides of caller-supplied controls. Reuse001's guarded period
   construction internally. Do not add fee roll/EOM/lag/window modes merely to
   mirror the full coupon schedule surface. Unsupported contract recipes retain
   their product owner rather than receiving a silent substitute.

2. **Preserve an independent original fee grid and termination context.**
   Compile this recipe over the builder's explicit issue→raw-maturity horizon,
   retaining each fee parent's nominal phase, original references, stub status,
   S<E, payment date P and complete001 day-count context. Coupon/payment program
   cuts do not restart fees, and fee dates do not create amortization cadence.
   This selected full-horizon fee leg terminates at its unadjusted raw-maturity
   endpoint; carry that declared fact only to queries reaching it. It is not
   inferred from the last surviving emitted row, a balance cut or003's separately
   selected actual coupon/principal terminal E. Keep parents distinct when P
   coincides. Calendar/date/grid/resource/context errors propagate; an invalid
   or empty periodic horizon is not a successful no-fee instruction.

3. **Require covered economic history for every actual fee query.** Use003's
   chronological exact stock in the periodic parent's currency. Opening needs
   the validated post-phase stock at S; Integrated needs coverage over [S,E_fee).
   Do not require unused future balances for Opening. Include all admitted
   start-date phases. No live closing-balance fallback,
   cash-date substitute or partial history. A movement at E_fee does not change
   the preceding fee. The compiled plan's history coverage must include every
   queried fee boundary even when it differs from coupon/principal E.
   A complete plan can explicitly carry its post-redemption/residual stock
   through the required query horizon; do not truncate/re-anchor the fee or
   infer additional movements. If that coverage is unsupported, error. Fixed
   dated cash requires no principal-history query.

4. **Name and require the two balance methods precisely.** Replace
   PointInTime with Opening and TimeWeightedAverage with Integrated in
   FeeAccrualBasis, without aliases or Default. Opening samples the post-phase
   stock B(S) once and holds its selected fee base for the whole parent.
   Integrated samples each actual constant-stock interval. Neither uses closing
   balance, settlement cash or input order. Define X(B) below and r=bp/10000:
   Opening fee is Q(X(B(S))*r*A_E); Integrated fee is
   Q(sum_i X(B_i)*r*(A(e_i)-A(s_i))). No division by A_E is required.

5. **Use the fee parent's cumulative contractual-accrual measure.** Define
   A(t)=001 period_accrual(original_fee_parent,S,t) with frozen full context.
   Validate, admit each finite nonnegative model prefix once to Decimal under
   core012, cache it, and use exact differences. Require monotone prefixes;
   never clamp negative weights or re-anchor direct day counts at stock changes.
   Weights telescope to admitted A_E. Retain full ICMA composite references and
   ACT365L context. Integrated means contractual-accrual weighted, not necessarily
   calendar-day averaged: OneOne places weight1 in the first nonempty interval
   and0 later. Legitimate zero weights/full accrual remain valid after complete
   active-input validation. Redundant equal-stock history entries do not create
   financial events or extra rounding.

6. **Define the drawn and undrawn bases exactly.** Drawn uses nonnegative
   covered stock B. Undrawn requires exact static Money commitment K>=0 in the
   periodic parent's currency, including validated K=0, and uses max(K-B,0).
   Clip EACH sampled interval before weighting; do not clip an averaged drawn
   balance. Preserve K-B transiently exactly rather than requiring its rounded
   Decimal restoration. B>K produces zero undrawn base by this selected formula;
   it is not permission to borrow above a limit. This static K applies over the
   selected whole fee horizon. Dynamic commitments, LC usage, reserves, excluded
   drawings and commitment expiry retain their product owners; do not infer them
   from outstanding stock or from a coupon's terminal date.

7. **Keep signed exact annual basis-point quotes.** bp is Decimal annual bp,
   including fractional bp and negative rebates; 1bp=1/10000 annual decimal rate.
   Do not quantize through the whole-integer core Bps type or infer a financial
   rounding rule. The schedule's nonnegative base/weight and quote determine
   sign; no fee floor, absolute value or rebate suppression. Product-specific
   nonnegative fee restrictions remain the actual product owner's policy.

8. **Evaluate the complete exact fee and restore Money once.** Use the exact
   transient rational bp/10000, bases, admitted prefix differences, products and
   whole-parent sum, then core012's nearest-even greatest-representable-scale
   Q once. Do not require a separate bounded Decimal annual-rate stage: a tiny
   rate can have a representable fee with a large base. Internal contributions
   need not separately fit Money. Remove magnitude-pair multiplication as a
   purported precision guarantee, per-piece restoration and f64 Money detours.
   Final overflow or nonzero-to-zero underflow errors; exact-zero results are
   valid. Reuse the same core exact evaluator authority as005, without adding
   a public arbitrary-precision mode. No currency-minor-unit rounding is implicit.

9. **Emit one periodic cash fee per original parent, including zero.** Store
   the one complete amount in one CFKind::Fee row at P, preserving parent/program
   identity. Internal history/context partitions create no extra cash rows.
   Retain enabled zero and signed rows, and do not merge distinct parents or
   programs merely because their dates/amounts match. Fee cash has no principal
   effect or capitalization under core015; a negative fee does not fund stock.
   Validate every active input and required context even when bp, base or A_E
   is zero; numerical underflow must not become an omitted zero fee.

10. **Keep cash metadata and calculation provenance truthful.** Retain exact
    bp, base/method, full fee-parent context and history/calculation provenance
    with the owner. Populate optional CashFlowAccrual only when one supported
    complete slice truthfully describes the row; omit false single-slice fields
    for composite parents, retaining the full owner context. Fee rows have no
    reset or projected-index quote. Any checked binary64 annual-rate metadata
    is a nonauthoritative view of exact bp/10000, never fed back into the amount.
    Do not force that view through an intermediate Decimal annual-rate stage.

11. **Treat Fixed as literal independent dated Money.** Keep the exact supplied
    signed amount, currency and supported payment date, including exact zero
    and duplicates as separate instructions. Apply no BDC, lag, implicit FX or
    minor-unit rounding. Remove the raw-maturity ceiling: a literal cash date
    may precede issue/history opening or follow raw maturity/principal terminal.
    This does not extend principal-event eligibility or establish product fee
    entitlement. Preserve currency rather than silently convert it; aggregate/
    reporting consumers must use their own declared currency rules. Fixed fee
    cash has no principal effect and requires no balance lookup.

12. **Make tiers a strict literal whole-input step selector.** Keep
    evaluate_fee_tiers standalone. Require supplied Decimal utilization and
    every Decimal threshold in[0,1], with strictly increasing thresholds in the
    supplied order; do not sort, deduplicate, clamp or infer utilization from B/K.
    Validate the whole table and utilization before selecting or taking empty
    shortcuts. Choose the highest threshold <= utilization, inclusively at
    equality. Its exact signed bp applies to the whole input, not marginal
    bands. The owner supplies the normalized utilization authority; this API
    does not promise an exact economic ratio from an already rounded input.

13. **Distinguish no applicable tier from an explicit zero quote.** Return
    Result<Option<Decimal>>: None for an empty table or utilization below the
    first threshold, Some(bp) for a selected tier, including Some(0). Invalid
    domain/table data errors even when no tier would match. Remove implicit
    no-match0. The actual product owner must deliberately select no-fee or
    missing-coverage behavior; generic tier lookup supplies no fallback. This
    package does not automatically connect tiers to PeriodicBp, reprice daily,
    average utilization, define K=0 ratios or admit utilization above1.

14. **Preserve phase and failure boundaries.** Fee evaluation remains in003's
    fee phase after coupon/PIK/principal/amortization and before redemption,
    while each amount queries its own half-open historical interval. Stored
    start-stock authority includes all applicable start-date closing phases;
    current end-date live stock is not that authority. Fees do not mutate stock
    or create future capitalization. Admit configuration atomically and publish
    only a complete successful local build; failed calendar/context/history/
    currency/arithmetic/metadata work cannot leak partial fee rows. External
    caller/provider effects are outside local rollback.

15. **Use one guarded Rust admission path across existing surfaces.** Validate
    fixed/periodic/base/tier payloads before zero omission or selection; remove
    deferred/sticky mutation errors and route the fee setter through fallible
    atomic admission. Remove omitted-field defaults and old enum names, using
    explicit Opening/Integrated across Rust, JSON, Python and WASM where exposed.
    Reconcile existing specs/constructors/accessors, stubs, facade/schema/parity,
    examples and strict restored state, rejecting unknown, missing required,
    malformed or inapplicable data without compatibility aliases. Keep tier
    lookup Rust-only where currently excluded; no new host framework is needed.

16. **Reconcile actual fee consumers without approving their other models.**
    Update term-loan builder usage-fee construction and actual revolver tier
    callers for the changed basis names, explicit inputs and nullable result.
    Remove the revolver flat-fee constructor's conversion-error-to-zero quote
    substitute; preserve the actual quote admission failure.
    Existing product fee floors/nonnegative validation, dynamic commitments,
    LC/default/exercise/payment rules retain their owners. The term-loan custom
    commitment-fee path is separate from shared PeriodicBp emission; do not claim
    it reconciled merely because this kernel changes, or silently reinterpret
    its cumulative-drawn/outstanding choices. Product use of this limited recipe
    must match its actual clauses or report unsupported behavior. Schedule
    lifecycle/reporting/entitlement remain012–015 and product decisions.

17. **Keep qualification scoped to the selected operators.** Independently
    qualify full fee grids/contexts and resource/date boundaries, start/end
    event phases, missing/right-extended history, opening versus integrated
    bases, per-interval clipping and overdrawn/zero commitment, nonadditive/zero
    day counts, exact tiny/large/negative/zero amounts, whole-parent restoration,
    metadata and duplicate-date identity. Include tier equality, all domain
    boundaries, empty/below-first versus selected zero, invalid masked tables,
    product-specific no-match policy and host/schema parity. Bounded examples
    and contract pointers support these distinctions, not an executed engine,
    universal market convention or a complete product fee implementation.

## Independent bounded reference checks

On2026-11-09 an inline independent Python Fraction calculation against the Decimal restoration
profile, without
importing or running production code, checked these definitions:

- Fixed US30U/360 EOM,29January→28February2025, cut31January: parent count29,
  direct pieces2+28=30, original-parent prefix weights2 and27. At3600bp with
  stock100→101, the selected integrated fee is2.927; direct re-anchored pieces
  give3.028. Constant stock100 gives2.9, as does Opening at100. These are ideal
  rational day-count references; actual admitted binary64 prefixes can alter
  final digits and retain their stated core012 admission limits.
- Commitment100, stock50 then150 with equal half-year weights,100bp: per-interval
  undrawn bases50 and0 give integrated fee0.25. Clipping the average drawn stock
  instead incorrectly gives0; Opening deliberately gives0.50.
- Base1e28, quote1e-28bp and A_E1 give fee0.0001, despite annual rate1e-32 being
  outside bounded Decimal storage. Base1e-28 at1bp instead gives true fee1e-32,
  requiring final nonzero-to-zero failure. Contributions0.4e-28+0.4e-28 restore
  once to1e-28; independent per-piece restoration loses or rejects both pieces.
- Tier thresholds0.25→25bp,0.5→0bp select None below0.25, Some(25) at0.25 and
  Some(0) at0.5. Unsorted/duplicate/out-of-domain thresholds and out-of-domain
  utilization remain errors, including with empty/no-match input.

These are reference arithmetic and semantic checks, not production tests,
contractual fee qualification or validation of product normalization/model paths.

## Observed algorithms and controls

- Fixed fees emit once at the exact supplied date, with no BDC or lag. Zero amounts are suppressed. Dates after raw maturity fail; pre-issue dates are accepted. The fixed amount's currency is retained.
- Periodic fee specifications supply tenor, day count, BDC, calendar, stub and annual bp. Compiler uses one full-horizon schedule with unadjusted accrual, no EOM, no roll rule and no payment lag. Default stub is short-front. Empty schedules or unknown calendars fail.
- PointInTime chooses the balance at the **start of the fee period**, then uses it for the full accrual interval. TimeWeightedAverage partitions at economic balance changes and emits one fee per nonzero segment using that segment's contractual day-count fraction.
- Drawn base is outstanding `B`. Undrawn base is `max(commitment-B,0)` and requires commitment currency to match the builder currency. No negative-commitment rejection is imposed by this branch; a negative commitment ordinarily produces zero undrawn fee.
- Fee amount is `base × bp × 10^-4 × year_fraction`. Checked Decimal multiplication pairs smallest and largest magnitudes at each step to reduce intermediate overflow/underflow. Nonrepresentable final/intermediate arithmetic errors. Negative bp fees are permitted and emit negative amounts. Flow rate records `bp×10^-4`; each flow carries its own accrual metadata.
- `evaluate_fee_tiers` requires strictly ascending thresholds, then selects the last tier with `utilization >= threshold`. It returns zero below the first threshold or for an empty table. The selected rate applies to the whole utilization input; this is not marginal-band integration. Decimal utilization, thresholds and quotes are not constrained to `[0,1]` or nonnegative values by this function.

## Observed ordering, alternatives and limitations

Emission runs after same-day coupon capitalization, principal events and amortization, but period calculations use history on `[start,end)`, so closing-date movements do not retrospectively change the preceding fee base. Issue-date amortization updates the initial history before later fee accrual.

Supported alternatives are opening-balance and integrated-balance fees. Ending-balance fees, marginal tier bands, dynamic commitments and fee-specific payment lags are not implemented in these specs. No currency minor-unit rounding per fee is configured here. Caller-side checks must establish product-specific utilization/commitment domains.

## Evidence

- [fees spec/tier selector](../../../../finstack-quant/cashflows/src/builder/specs/fees.rs), [fee compiler](../../../../finstack-quant/cashflows/src/builder/compiler.rs), [fee emission](../../../../finstack-quant/cashflows/src/builder/emission/fees.rs).
- Inspected hand formula `b14_fee_uses_contract_daycount_on_each_balance_segment` in emission/fees uses ACT/ACT and an extinguished balance to expect `1,000,000×0.005×30/365`. Located PointInTime/TWA constant-balance equivalence and changing-balance cases there. [principal_credit_regressions.rs](../../../../finstack-quant/cashflows/tests/principal_credit_regressions.rs) locates large-notional/tiny-quote precision tests. Tier source tests check ordering and zero-below-threshold behavior.
- No universal fee-calculation standard or contract fixture establishes the default basis. Existing tests substantiate formulas/branches but do not supply product adoption authority.
- [World Acceptance2015 amendment, section3.1](https://www.sec.gov/Archives/edgar/data/108385/000010838515000047/exhibit101_62215.htm) uses actual/360 average daily unused commitment with monthly payment terms. [Wintrust2025 amendment, section4.3](https://www.sec.gov/Archives/edgar/data/1015328/000101532825000211/ex1025thamdtoarcreditagree.htm) has average daily unused commitment and quarterly payment terms. These primary contractual examples support explicit selection, not a universal basis/payment rule or a claim that this limited builder recipe implements either agreement completely.
- [Revolver types](../../../../finstack-quant/valuations/fixed-income/src/revolving_credit/types.rs) calls the shared tier selector and applies its own rate/floor policies. [Term-loan cashflows](../../../../finstack-quant/valuations/fixed-income/src/term_loan/cashflows.rs) uses builder fee construction and a separate custom commitment-fee path. These are actual reconciliation owners, not evidence of approved product policies.
- Existing [Python fee specs](../../../../finstack-quant-py/src/bindings/cashflows/builder/specs/fees.rs), [WASM cashflow API](../../../../finstack-quant-wasm/src/api/cashflows) and [parity ledger](../../../../finstack-quant-py/parity_contract.toml) expose existing specs/enums while the standalone tier helper remains Rust-only. Reconcile those surfaces without introducing new host classes. Source/consumer reviews and the bounded independent checks above do not execute production functions.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
