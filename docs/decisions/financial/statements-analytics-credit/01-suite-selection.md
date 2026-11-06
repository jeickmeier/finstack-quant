# STATEMENTS-ANALYTICS-CREDIT-01: Mapping-driven check selection and pass semantics

- Decision status: **Draft — observed implementation; adoption pending**.
- Implementation status: **Observed** for the stated source scope; discrepancies are identified below.
- Validation status: **Evidence inspected or located, not executed**.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Typed node mappings assemble three-statement, credit-underwriting and LBO check suites. This records the implemented selection and thresholds. Comments describe a broad mechanical plausibility screen; the precise ranges are library policy choices, not demonstrated credit or accounting acceptance criteria.

## Scope and input domain

A `ThreeStatementMapping` names required cash, retained-earnings and net-income nodes plus asset/liability/equity collections and optional reconciliation inputs. `CreditMapping` names debt, EBITDA, interest and optional FCF/cash/burn nodes and thresholds. Unknown serialized fields are rejected. The check context supplies ordered model periods and computed values.

## Algorithm

1. Three-statement construction always adds balance-sheet articulation and retained-earnings roll-forward from the statements crate. It adds cash reconciliation only with total cash flow; depreciation only with PPE, depreciation and capex; capex only with at least one addition component; dividends only with both cash-flow and equity nodes; interest only with a direct capital-structure interest node or debt-balance inputs; working capital only with a CF movement and at least one component; tax only with tax and pretax income.
2. The suite adds growth checks for every `all_nodes()` entry, with bounds −0.50 to +1.00, and a non-finite sweep over those entries. The mapped list is not deduplicated. Required-value checks cover cash, retained earnings and net income rather than every mapped optional input.
3. Credit selection always adds debt/TTM-EBITDA leverage and EBITDA/current-interest coverage; default leverage warning/error ranges are [0,6]/[0,10], coverage floors 1.5/1.0. Optional FCF uses two/four consecutive negative observations. Optional cash plus burn enables 12/6-month runway thresholds.
4. Credit trend checks target the EBITDA level as increasing-is-good and debt level as decreasing-is-good. They do not compute trend in leverage or coverage ratios, despite the suite's descriptive wording. The non-finite sweep covers debt, EBITDA, interest and optional FCF, excluding cash and burn.
5. The LBO suite merges both check groups and overrides the credit warning leverage range to [0,8], including a caller-specified warning range. Each check's `passed` flag generally means no Error-severity finding; Info and Warning findings can coexist with a pass.

## Selection and controls

The presence of mapping inputs controls registration, not whether sufficient observations exist at execution. Per-check implementations decide skips, units and tolerance. Core check-runner policy and accounting identities are owned by the statements-contracts scope.

## Conventions

Growth bands are decimal period changes, leverage/coverage are ratios, runway is months, FCF thresholds are observation counts. Renderer text and HTML preserve report values and severity; numeric formatting and escaping do not recompute financial results.

## Limitations, failure and fallback

Registered checks can execute no comparisons and still pass. There is no per-check evaluated/skipped completeness count in these implementations. Optional cash/burn non-finite inputs can evade the credit suite's dedicated non-finite sweep. Mapping duplicates can repeat plausibility findings. Defaults do not vary by sector, period cadence or accounting jurisdiction.

## Alternatives and tradeoffs

An explicit coverage/completeness result would distinguish evaluated passes from skips; broader required-data checks would be stricter but require optional-input policy. Ratio-trend checks and level-trend checks answer different credit questions. Keeping caller LBO overrides versus imposing a canonical band is an API policy choice.

## Evidence

[suites.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/suites.rs), [mappings.rs](../../../../finstack-quant/statements-analytics/src/analysis/checks/mappings.rs) own selection. [suite_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/suite_tests.rs) locates registration, optional-input and LBO merge tests; [renderer_tests.rs](../../../../finstack-quant/statements-analytics/tests/checks/renderer_tests.rs) covers presentation. These are integration/contract evidence, not empirical validation of thresholds. Shared runner ownership: [statements checks](../../../../finstack-quant/statements/src/checks).

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-credit-checks-and-impairment).
