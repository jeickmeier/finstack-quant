# CORE-CONVENTIONS-002: Coupon reference periods for ACT/ACT ICMA and ACT/365L

- Decision status: **Accepted — complete coupon-context package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, scoped to `dates/daycount/{act_act,other,context}.rs` and `Tenor::to_years_with_context`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent formula-example checks passed; production tests and vendor fixtures were not executed or regenerated.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Coupon-sensitive accrual uses explicit contractual reference boundaries. The accepted package defines contained ICMA accrual and ISDA coupon-end-year ACT/365L; the observed implementation below retains branches requiring reconciliation. The source cites [ICMA Rule Book](../../../REFERENCES.md#icma-rule-book). Selection for a particular instrument remains with its contractual methodology record.

## Accepted decision: coupon-context day counts

Accepted on **2026-11-07** after whole-record review. The complete package
selects the financial definitions, supported context and helper boundaries
together. Runtime reconciliation and independent qualification remain open;
the original implementation observations remain separate below.

1. **Use explicit contractual periods in the existing context.** Retain
   the canonical `ActActIsma` and `Act365L` names and the existing
   `DayCountContext` / `DayCountContextState` representation. ICMA requires
   an explicit coupon frequency and one unadjusted regular nominal reference
   period, including for a complete regular coupon. ACT/365L requires the
   enclosing contractual calculation/coupon period; frequency is not needed
   by its selected formula. Do not infer coupon frequency or periods from
   maturity tenor, elapsed days, currency or payment dates.

2. **Adopt one contained-period ICMA calculation.** Let `(a,b)` be the
   supplied regular nominal reference period, `F` the number of regular
   coupon payments per year, and `a <= start <= end <= b`. Calculate
   `actual_days(start,end) / (F * actual_days(a,b))`, with elapsed days
   measured over `[start,end)`. A full reference period contributes `1/F`;
   a short coupon or partial accrual uses the same full nominal-period
   denominator. The actual short-stub boundaries must not replace the
   regular reference period. This follows the nominal-period formula in
   [ISO 15022 MICO A006](https://www.iso20022.org/15022/uhb/mt527-33-field-22a.htm)
   and the [official Strata ICMA description](https://strata.opengamma.io/day_counts/#actualactual-icma).

3. **Constrain and cross-check the ICMA frequency.** Support regular
   frequencies `1M`, `2M`, `3M`, `4M`, `6M` and `12M`, with `1Y` equivalent
   to `12M`: for nominal month count `m`, `F = 12/m`. Reject day/week,
   multi-year and other month frequencies in this ICMA API. This is an
   explicit supported domain, not a claim that every possible contractual
   coupon must use these frequencies. Require a strictly increasing
   reference pair spanning one declared `m`-month regular nominal period;
   cross-check the year/month separation against `m`, rather than rounding
   elapsed days to infer months. The contract/schedule supplies the actual
   nominal roll dates; the kernel neither reconstructs a roll rule from two
   dates nor proves that caller-supplied dates match an external contract.

4. **Compose stubs from known nominal periods.** For long first/last
   coupons or an accrual spanning several periods, the consuming schedule
   or cashflow layer splits the requested interval against the actual
   contractual nominal reference periods and sums the contained-period
   contributions. The slices must cover the requested span exactly once,
   without gaps or overlaps. Keep the coupon frequency consistent with each
   supplied reference. Remove the frequency-only shortcut, rounded-month
   inference and automatic quasi-grid extrapolation from the core kernel.
   Two annual February-28 endpoints can describe either a fixed-day or an
   end-of-month schedule with different future leap-year dates; they are
   insufficient to choose that schedule. No new roll/context framework is
   needed for the single-period primitive. Schedule generation itself
   remains owned by [008](008-schedule-generation.md).

5. **Select the ISDA coupon-end-year ACT/365L definition.** For a slice
   contained in the supplied contractual calculation/coupon period, divide
   actual elapsed days by 366 if that period's later end date is in a leap
   year, otherwise 365. The period's denominator stays fixed for every
   partial slice. Apply this rule to every coupon frequency; remove the
   existing annual leap-day exception and the frequency requirement.
   [ISDA's October 2022 response, A009, printed pages 22–23](https://www.leiroc.org/publications/gls/isda-response-09-22.pdf)
   identifies this rule under the amended 2006 definitions and 2021 section
   4.6.1(ix). [ISO 15022 A009](https://www.iso20022.org/15022/uhb/mt527-33-field-22a.htm)
   corroborates the leap-year denominator and identifier. A008 identifies
   ISDA Actual/Actual, not ACT/365L. This decision deliberately changes the
   observed frequency-dependent variant; it does not retain it through an
   alias or fallback. Contract-specific selection still belongs to the
   instrument record.

6. **Keep reference dates, accrual ends and payment dates distinct.**
   ICMA's reference dates are unadjusted nominal coupon dates. For ACT/365L,
   the supplied enclosing period uses the contractual calculation-period
   boundaries, including any adjustment explicitly applicable to those
   boundaries. Its later end is not automatically the delayed cash-payment
   date or the end of a partial query. An accrual crossing multiple ACT/365L
   periods must be split using each period's own end-year denominator.
   Do not silently extend a reference period to accommodate an adjusted
   query endpoint.

7. **Validate before zero and preserve chronological context.** Apply
   [001's accepted structural context boundary](001-actual-day-counts.md#accepted-decision-actual-day-conventions)
   to runtime, constructed and restored state. Then validate the selected
   convention's required frequency/reference/enclosing period and containment
   before returning equal-date zero. Valid but irrelevant context remains
   allowed; ACT/365L does not require an ICMA-compatible frequency. The
   unsigned API rejects reversal; the signed API calculates the chronological
   interval with the same valid reference/enclosing period and negates it.
   Do not swap coupon boundaries or invent replacement metadata. Invalid,
   missing, inconsistent or out-of-period context is an explicit input
   error; unavailable required dates and failed/nonfinite calculations are
   errors. There is no ACT/365F substitution, clipping, fabricated zero or
   denominator floor. Count days exactly before floating-point division;
   complete-period weights and partial ratios define results, not obsolete
   summation order. The old 512-period traversal restriction disappears with
   grid inference, without a new configurable horizon cap.

8. **Pass actual context through the tenor bridge and all hosts.** Extend
   the same canonical `Tenor::to_years_with_context` operation to receive
   caller-supplied `DayCountContext`, and delegate calculation with that
   context. Remove its invented `frequency = self` and coupon boundaries.
   The optional endpoint-adjustment calendar and the accrual context retain
   their distinct roles; one must not silently supply the other. If optional
   endpoint adjustment moves the measured end before `as_of`, return a date
   error. Equality returns zero only after the selected context is valid.
   An ICMA endpoint outside its supplied nominal reference requires explicit
   schedule slicing, not inferred extra periods. Reconcile Rust, Python,
   WASM, wire validation and public documentation in the same implementation
   slice, using the existing context type without parallel helpers or
   compatibility paths. XIRR's supported convention/context contract remains
   with [017](017-irr.md). Methodology approval does not execute or qualify
   these runtime changes.

## Boundary examples for the accepted package

These are independent calendar-arithmetic examples, not executed production
tests or regenerated vendor output. All periods are explicitly supplied.

| Context and request | Required result |
| --- | --- |
| ICMA `6M`, nominal `2025-01-15 → 2025-07-15` (181 days), full period | `1/2`. |
| Same nominal period, short accrual `2025-03-15 → 2025-07-15` (122 days) | `122 / (2 * 181)`. |
| Long first coupon `2024-11-15 → 2025-07-15`, split at `2025-01-15`; prior nominal period `2024-07-15 → 2025-01-15` (184 days) | `61 / (2 * 184) + 1/2`, composed from two explicit contained slices. |
| ICMA `6M` with a three-month reference period, or any ICMA request missing its reference | Input error. |
| ACT/365L whole annual period `2023-01-01 → 2024-01-01` | `365/366`; the observed annual branch returns `1`. |
| ACT/365L whole annual period `2024-01-01 → 2025-01-01` | `366/365`; the observed annual branch returns `1`. |
| ACT/365L enclosing period `2023-07-01 → 2024-07-01`, partial request `2023-07-01 → 2023-12-01` | `153/366`, retaining the enclosing period's denominator. |
| Equal dates with missing required period/frequency, or outside the supplied period | Input error rather than a zero shortcut. |

## Observed algorithm

**ACT/ACT ICMA/ISMA:** frequency is mandatory even when an explicit reference period is supplied. Without reference boundaries, only month/year tenors are supported. Generate `start + k * tenor_months` from the original start, for at most 512 steps; accept only if a boundary equals end, returning the sum of `tenor_months/12`. An end between grid points requires an explicit coupon period.

With reference boundaries `(a,b)`, require `a < b`; infer nominal months `m = round(12*(b-a).days/365)` and reject zero. Use the endpoint with the larger day-of-month as the unclamped anchor (start wins ties). If both endpoints are month-end, snap generated boundaries to month-end. Regenerate boundary indices 0 and 1 and reject if they do not reproduce `(a,b)`. Walk backwards/forwards to the period containing start, then sum `overlap_actual_days / reference_segment_actual_days * m/12` over quasi-coupons. Absolute boundary indices must remain below 512; date and offset overflow error. In this branch the supplied frequency is required but does not determine `m` or get cross-checked against it.

**ACT/365L:** require frequency and an enclosing strictly increasing coupon period with the requested accrual inside it. Annual means exactly `1Y` or `12M`: use denominator 366 if February 29 is in `(coupon_start,coupon_end]`, else 365. Every other frequency uses 366 if coupon end is in a leap year, otherwise 365. Return actual accrual days divided by that coupon denominator. A partial accrual keeps the enclosing coupon's denominator; multi-coupon accrual requires external slicing.

## Selection, conventions and controls

`DayCountContext` carries optional frequency, coupon period, calendar, basis and termination flag. Boundaries for ICMA are unadjusted nominal coupon dates, not adjusted payment dates. For `Tenor::to_years_with_context`, ICMA reference end is the unadjusted tenor end, while the measured accrual end is optionally business-day adjusted; other conventions receive the adjusted end as coupon end. The helper returns zero if adjustment moves end to/before start.

## Limitations and failure

ICMA reference periods inferred by rounded actual days have no independent frequency-consistency validation. The explicit-reference branch permits a supplied day/week frequency because that frequency is not used after presence checking. ACT/365L similarly treats any nonannual tenor as the nonannual rule. Neither convention silently switches to ACT/365F. Both become unusable for nonempty XIRR date intervals through the context-free XIRR signature; see [017](017-irr.md).

## Alternatives and tradeoffs

The observed frequency-only and inferred quasi-coupon paths are replaced by the accepted explicit single-period contract; callers compose known nominal periods for longer accruals. Exact schedule metadata avoids ambiguous roll inference and creates a caller obligation. The selected ACT/365L end-year rule removes the observed annual exception. No competing schedule engine or compatibility path is adopted.

## Evidence

[ICMA implementation](../../../../finstack-quant/core/src/dates/daycount/act_act.rs), [365L implementation](../../../../finstack-quant/core/src/dates/daycount/other.rs), [context](../../../../finstack-quant/core/src/dates/daycount/context.rs), [tenor bridge](../../../../finstack-quant/core/src/dates/tenor.rs). Inspected inline tests include `act_act_isma_reference_period_preserves_eom_anchor_across_stubs`, `act_act_isma_coupon_period_mid_coupon_accrual`, and the three February-29 ACT/365L boundary cases; [day-count integration tests](../../../../finstack-quant/core/tests/dates/daycount.rs) provide additional expected fractions. [Stored reference cases](../../../../finstack-quant/core/tests/golden/data/daycount_quantlib.json) are finite fixtures, not independent verification of every accepted context.

The observed input behaviors above do not override the accepted contract.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
