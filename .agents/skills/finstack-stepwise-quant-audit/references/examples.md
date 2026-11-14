# Worked Example Audits

Two condensed examples showing the expected output shape and judgment level.
Audit all steps internally, but report only Blocker and High findings. Passing
steps and lower-severity uncertainty are omitted, not promoted to meet the
reporting threshold.

## Example 1 — Bond dirty price walkthrough (Fail found)

Subject code (illustrative):

```rust
fn dirty_price(bond: &Bond, curve: &Curve, trade_date: NaiveDate) -> f64 {
    let settle = trade_date; // settlement lag not applied
    let accrued = accrued_interest(bond, settle);
    let pv: f64 = bond
        .cashflows_after(settle)
        .iter()
        .map(|cf| cf.amount * curve.df(trade_date, cf.date))
        .sum();
    pv + accrued
}
```

Expected audit (abridged):

```markdown
## Subject
Walkthrough of `dirty_price` for a fixed-coupon bond: settlement, accrual,
cashflow selection, discounting, and price assembly within one function.

## Findings

### [High] Step 1 — Missing settlement lag
- **Standard:** Corporate/govvy bonds settle T+1 or T+2 on the relevant
  calendar (fixed-income-standards.md, settlement section).
- **Evidence:** `pricer.rs:2` — `let settle = trade_date;` uses trade date
  directly, no lag, no calendar.
- **Verdict:** Fail
- **Impact:** Accrual and cashflow cutoff shift by the settlement lag; a coupon
  between trade and settlement can be included despite not belonging to the
  buyer.
- **Fix:** Compute `settle = calendar.advance(trade_date, lag)` with the lag
  and calendar confirmed from bond reference data; thread it through accrual,
  cashflow selection, and discounting. Fix Steps 1 and 4 together.

### [High] Step 4 — Wrong discount anchor
- **Standard:** Bond PV is discounted to the settlement date; discounting to
  trade date misstates price by the lag's carry.
- **Evidence:** `pricer.rs:7` — `curve.df(trade_date, cf.date)` anchors on
  trade date.
- **Verdict:** Fail
- **Impact:** Settlement prices systematically include the wrong carry over
  the lag, biasing trade valuation and P&L.
- **Fix:** Anchor discounting at `settle`: `curve.df(settle, cf.date)`.
```

Judgment notes:

- The internal step map is settlement, accrued interest, cashflow selection,
  discounting, and price assembly; only reportable steps appear in the output.
- Step 2 is a *conditional Pass*: its own logic is right, but its input is
  wrong. Omit it from the report and attribute the defect to Step 1.
- Fold the cross-step dependency and reference-data question into the relevant
  finding rather than adding separate sections.

## Example 2 — DV01 bump path (lower-severity uncertainty omitted)

Situation: a `dv01` function bumps the zero curve +1bp and reprices, one-sided.
The missing policy documentation creates uncertainty, but there is no evidence
of a material mismatch with downstream consumers or another reportable defect.

Internal assessment:

- Both quote-bump-and-rebootstrap and zero-curve bumps are used in practice;
  the choice matters for bucketed risk (algorithm-standards.md, risk section).
- `position_risk.rs:88-95` bumps zero rates directly without documenting the
  policy. The numbers are internally consistent.
- Missing documentation alone does not establish Blocker or High impact.
  Suppress the observation rather than promoting it or adding an open question.

Expected output:

```markdown
## Subject
Audit of the zero-curve DV01 bump path in `dv01`, limited to bump specification
and repricing.

## Findings
No Blocker or High-severity findings identified within the audited scope.
```

Judgment notes:

- An Uncertain verdict is reportable only when evidence supports a plausible
  Blocker or High impact, such as a demonstrated mismatch with the risk
  convention required by downstream hedging.
- No reportable findings does not certify the entire risk pipeline.
