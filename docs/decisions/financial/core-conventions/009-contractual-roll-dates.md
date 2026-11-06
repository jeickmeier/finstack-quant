# CORE-CONVENTIONS-009: IMM, CDS quarterly/semiannual rolls and option expiry anchors

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to roll-date functions in `dates/imm.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Shared helpers supply unadjusted contractual anchors. They distinguish quarterly IMM futures, quarterly CDS coupons, semiannual CDS roll/maturity cycles, quarterly futures-option expiry and monthly equity-option Friday anchors. They do not infer exchange-specific exercise or settlement policy.

## Algorithm and actual selection

Third Wednesday/Friday uses the nth-weekday kernel. Quarterly months are March, June, September, December. `next_imm`, `next_cds_date` and `prev_cds_date` are strictly after/after/before the supplied date. Internal IMM on-or-after/on-or-before helpers are inclusive. `is_imm_date` requires quarterly month, Wednesday and day 15–21; `is_cds_date` requires quarterly month and day 20.

`prev_cds_semiannual_roll` returns March/September 20 on or before input. `next_semiannual_cds_maturity` returns June/December 20 on or after input. A generic month scan iterates candidate months in direction order, moves years as needed and errors at representable date boundaries.

`imm_option_expiry(month,year)` is third Wednesday minus five calendar days, producing the preceding Friday. It accepts any month; `next_imm_option_expiry` searches quarterly months strictly after input. `next_third_friday` searches all months strictly after input. Neither moves a Good Friday or other closure; caller-specific holiday adjustment is separate.

## Conventions, controls and limitations

Date granularity is one calendar day. Inclusive and exclusive variants are intentional and can shift a trade on roll day by a full cycle. No automatic pre-/post-Big-Bang or historical contract-regime selector exists in these primitives. `third_wednesday` alone permits nonquarterly months while `is_imm_date` does not.

## Alternatives and tradeoffs

The existing helpers support different contractual cycle anchors. Exchange-adjusted expiries, product-specific notice days and a combined trade-to-maturity algorithm would require additional selections; they are not implicit in the returned dates.

## Evidence

[Owning code and inline tests](../../../../finstack-quant/core/src/dates/imm.rs), including `prev_semiannual_roll_selects_mar_or_sep_on_or_before`, `semiannual_maturity_snaps_to_jun_or_dec`, `next_third_friday_remains_unadjusted_on_good_friday`, and date-boundary failures. [Schedule tests](../../../../finstack-quant/core/tests/dates/schedule.rs) exercise actual IMM caller behavior. [ISDA CDS model](../../../REFERENCES.md#isda-cds-standard-model) is relevant background; the inspected code does not establish an independent exchange rulebook citation for every option-expiry helper.

These date formulas alone do not certify a trade's contractual maturity or exercise deadline.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
