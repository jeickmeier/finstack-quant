---
name: finstack-stepwise-quant-audit
description: >
  Walk a quant finance process, algorithm, pricing path, risk workflow, or
  cashflow pipeline step by step and evidence-audit each stage against market
  standards and desk practice. Use whenever the user asks to go through a
  model, pricer, bootstrap, schedule builder, calibration, Greeks path, or
  valuation workflow one step at a time; to verify each step is industry
  standard and correct; to certify a pipeline before trusting its output; or
  to get findings plus recommended fixes for a multi-stage quant process.
  Prefer this over a broad review when the request is sequential walkthrough
  and per-step correctness, not a general code review. Report only Blocker
  and High-severity findings.
---

# Stepwise Quant Audit

Decompose a quant finance process into ordered steps. For each step, assess what
it does, cite the industry standard or desk convention it should satisfy, judge
whether the implementation matches, and recommend a concrete fix when it does
not. Audit every step, but report only Blocker and High-severity findings.

This is an evidence-based audit, not a test-writing exercise and not a generic
code review. Prefer standards, formulas, conventions, and existing evidence in
the repo over inventing new executable tests. Suggest tests only when a step
cannot be certified from inspection and citations alone.

## Relationship to sibling skills

- Use **this skill** when the user wants a sequential walkthrough with a
  verdict per stage in the analysis and only material findings in the report.
- Use `finstack-quant-finance-review` for broader pricing/risk/convention
  reviews, library assessments, or defect hunts that are not step-ordered.
- Use `finstack-senior-code-review` for style, simplicity, and over-engineering.
- Reuse market-standards and references under
  `../finstack-quant-finance-review/` rather than inventing parallel convention
  docs.

## When to use

- "Walk me through this pricer / bootstrap / schedule / risk path step by step"
- "Is each step of this algorithm industry standard?"
- "Audit this valuation workflow before we trust the numbers"
- "Check this cashflow or curve construction pipeline stage by stage"
- Any multi-stage quant process where a wrong intermediate step silently
  corrupts later results

## Core workflow

Complete these phases in order. Do not skip ahead to fixes before the step map
exists.

### 1. Scope the subject

Identify:

- Asset class (rates, FX, credit, fixed income, equity, commodities, cross-asset)
- Process type (pricing, cashflows/schedule, curve bootstrap, calibration,
  risk/Greeks, scenario, settlement, P&L explain)
- Boundary of the walkthrough (file, function, module, or documented workflow)
- Valuation date, currencies, curve roles, and day-count/calendar assumptions
  when they affect correctness

If the subject is ambiguous, ask one focused clarifying question. Otherwise
infer from code and proceed.

### 2. Build the step map

Break the process into an ordered list of atomic steps. Each step should be
small enough that a single standard or convention can judge it, but large enough
to be a meaningful stage (not every line of code).

Start from the canonical decomposition for the process type in
`references/step-map-templates.md` (bond pricing, swap pricing, curve
bootstrap, option pricing, Greeks path, schedule generation, calibration,
Monte Carlo, CDS). Adapt it to the actual code — merge steps the
implementation combines, split steps it separates — and audit the ordering
itself when it deviates from the template, since a nonstandard order is
sometimes the bug.

Typical step grains:

- Input validation and market-data selection
- Schedule / fixing / accrual construction
- Projection or discounting
- Payoff or cashflow aggregation
- Solver / bootstrap / calibration iteration
- Risk bump or Greek aggregation
- Output units, signs, and reporting conventions

Build the numbered step map first in the working analysis. Reported findings
refer to these step IDs; do not print the full map.

### 3. Audit each step

For every step, assess the following in the working analysis:

1. **Intent** — what this step claims to accomplish
2. **Standard** — the industry rule, formula, or desk convention it should
   follow (cite ISDA, market convention, QuantLib/Bloomberg practice, textbook
   formula, or the sibling `market-standards/` file)
3. **Evidence** — what the code/docs actually do (file + line when available)
4. **Verdict** — `Pass`, `Fail`, or `Uncertain`
5. **Fix** — if Fail or Uncertain-with-risk: a concrete recommended change

Evidence sources, in preference order:

1. Sibling market-standards files (rates, FX, fixed income, equity, algorithms,
   cross-asset checklist)
2. Authoritative external conventions (ISDA definitions, central-bank/RFR
   conventions, well-known desk practice)
3. Cross-checks against QuantLib / Bloomberg / published formulas when relevant
4. Existing tests, golden values, or comments in-repo that confirm intent
5. Explicit assumption stated as `Uncertain` when evidence is missing

Do not mark Pass on vibes. A Pass needs a cited standard and matching evidence.

Attribute each defect to the step that owns it, not to every step it
contaminates. A step whose own logic is correct but which receives a wrong
input from an earlier failed step gets a **conditional Pass** ("Pass, given the
Step N fix") rather than a Fail — this keeps the findings list actionable
instead of repetitive.

### 4. Check step-to-step contracts

After per-step audits, inspect interfaces between steps:

- Units and signs (decimal rate vs percent vs bp; clean vs dirty; paid vs received)
- Date and calendar handoffs (accrual end vs payment date; spot lag; stub rules)
- Curve role handoffs (projection vs discount; collateral vs funding)
- Missing or dropped intermediate quantities (fixings, notionals, FX resets)
- Error propagation (silent defaults, NaN continuation, wrong fallback)

Many production bugs live between steps, not inside them.

### 5. Report findings and fixes

Report only Blocker and High-severity findings, ordered by severity. Keep
passing steps, conditional passes, lower-severity observations, and immaterial
uncertainty out of the report, including summaries, questions, and appendices.
Do not promote a lower-severity concern just to include it. Report Uncertain
only when evidence supports a plausible Blocker or High impact, and state the
missing evidence without presenting the risk as a confirmed defect.

Fold material cross-step issues into the finding for the owning step.
Recommended fixes should be actionable (what to change, where, and why the
standard requires it). Do not implement code changes unless the user asks.

## Verdict guide

| Verdict | Use when |
|---------|----------|
| **Pass** | Evidence matches a cited industry standard or desk convention |
| **Fail** | Evidence contradicts the standard, or a required convention is missing/wrong |
| **Uncertain** | Standard is ambiguous, evidence is incomplete, or behavior depends on undocumented policy |

Reportable severity for Fail / material Uncertain:

- **Blocker** — wrong price, P&L, risk, or economics if left as-is
- **High** — convention or numerical issue that can materially misstate results in common cases

Omit lower-severity concerns, such as defensible nonstandard practices needing
documentation or clarity, naming, and documentation gaps that do not change
numbers.

## Output format

ALWAYS use this structure:

```markdown
## Subject
One sentence: what was audited, asset class, boundary, and any scope limitations.

## Findings
Only Blocker and High findings, ordered by severity.

### [Blocker | High] Step <ID> — <issue>
- **Standard:** ... (citation)
- **Evidence:** `path/to/file.rs:lines` — ...
- **Verdict:** Fail | Uncertain
- **Impact:** ... (include affected downstream steps when relevant)
- **Fix:** ... (include any question or missing evidence needed to resolve this finding)
```

If no findings meet the threshold, keep Subject and Findings and state:
"No Blocker or High-severity findings identified within the audited scope."
Do not list suppressed observations or imply that this certifies the entire
pipeline.

See `references/examples.md` for two condensed worked audits (a bond pricing
Fail and a suppressed DV01 uncertainty) that calibrate the reporting threshold
and output shape.

## Reference map

Open only what the asset class and process need. Paths are relative to this
skill directory.

### Local references
- `references/step-map-templates.md` — canonical step decompositions per
  process type, with the standards that usually decide each step's verdict.
  Open this when building the step map.
- `references/examples.md` — worked example audits showing reportable findings,
  suppressed lower-severity observations, and the output format in use.

### Market standards
- `../finstack-quant-finance-review/market-standards/rates-standards.md`
- `../finstack-quant-finance-review/market-standards/fx-standards.md`
- `../finstack-quant-finance-review/market-standards/fixed-income-standards.md`
- `../finstack-quant-finance-review/market-standards/equity-standards.md`
- `../finstack-quant-finance-review/market-standards/algorithm-standards.md`
- `../finstack-quant-finance-review/market-standards/cross-asset-checklist.md`

### Deeper references (as needed)
- `../finstack-quant-finance-review/references/pricing-models.md`
- `../finstack-quant-finance-review/references/risk-models.md`
- `../finstack-quant-finance-review/references/numerical-methods.md`
- `../finstack-quant-finance-review/references/numerical-regression.md`
- `../finstack-quant-finance-review/references/finstack-module-index.md`

## Practitioner heuristics

- Wrong day count, stub, or settlement lag at step 2 makes a perfect NPV formula
  at step 5 worthless — audit early schedule and market-data steps hard.
- Treat LIBOR conventions as legacy unless the code models fallback or
  historical contracts.
- Prefer naming the standard ("USD SOFR OIS, ACT/360, T+2") over vague claims
  ("looks conventional").
- If two desk practices exist, document both, pick the one the code implements
  (or should implement), and mark Uncertain only when the choice is material
  and unspecified.
- Recommend fixes that restore industry-standard behavior; avoid speculative
  refactors that do not change financial correctness.
- Keep the walkthrough honest about scope: if you only audited the floating
  leg, say so in Subject.
