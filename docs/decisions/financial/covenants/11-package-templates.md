# COVENANTS-11: Deal-package assembly and embedded consequence assumptions

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Actual package construction

`covenant_template` dispatches on four tagged variants and validates each input only as finite and nonnegative. Ratios are turns; debt yield and LTV are decimal fractions without an upper bound; capex/liquidity are untyped reporting-currency amounts. The constructors return ordered specs and do not infer a deal's governing documents.

| Package | Ordered tests and embedded policy |
| --- | --- |
| LBO standard | Quarterly maintenance debt/EBITDA (`max_debt_ebitda`, 30-day cure, +200bp rate); interest coverage (`min_interest_coverage`, 30 days, distribution block); fixed-charge coverage (`min_fcc`, 30 days, no consequence); annual capex (`max_capex`, inherited 30 days, no consequence). Leverage is constant unless the caller later attaches a schedule. |
| Cov-lite | Quarterly incurrence total and senior leverage (`max_total_leverage`, `max_senior_leverage`); annual descriptive negative covenant (`negative`) against additional secured debt without consent. All inherit 30-day cure and no consequences. The negative test is not numerically assessed. |
| Real estate | Quarterly maintenance DSCR (`min_dscr`, 30 days, 100% sweep); custom minimum debt yield (`min_debt_yield`, inherited 30 days, no consequence); custom maximum LTV (`max_ltv`, inherited 30 days, 50% sweep). |
| Project finance | Quarterly maintenance DSCR default (`min_dscr_default`, 60 days, Default); DSCR lockup (`min_dscr_lockup`, inherited 30 days, distribution block); liquidity (`min_liquidity`, inherited 30 days, no consequence); net debt/EBITDA (`max_net_debt_ebitda`, inherited 30 days, default denominator `ebitda`, no consequence). |

Frequency remains metadata; the engine will test annual capex on any caller date without constructing annual expenditure. Labels distinguish two DSCR tests and two custom CRE tests, but concatenating packages with duplicate labels can fail engine validation. Independently valid template input does not establish a consistent joint capital structure, reserve, or earnings convention.

## Observed contract difference and limitations

Project-finance documentation calls lockup DSCR the higher threshold, but implementation only validates its sign/finiteness; the inline valid-template test explicitly accepts default DSCR 1.2 with lockup DSCR 1.1. The asserted relationship is therefore not enforced. Total versus senior leverage thresholds likewise have no cross-input consistency check. The record does not infer that every such ordering must be prohibited; the intended contract must be settled.

CRE sweeps are separate setter actions in caller/history order, not a maximum-of-triggered-sweeps rule. Project default and CRE sweeps cannot simply be executed through the inspected TermLoan adapter without extra recovery/cash-flow inputs; see [COVENANTS-06](06-consequence-execution.md). A named package is not a market/legal standard certification. In particular, source phrases such as “Post-2015 ... standard” have no contract sample or empirical provenance in the inspected module.

## Rationale, alternatives and evidence

The code documents convenience assembly, distinct instance keys and different default versus lockup consequences. Per-agreement explicit specs or externally versioned templates would allow provenance and differing cure/consequence conventions; neither is selected automatically here.

Owning [template definitions/builders/tests](../../../../finstack-quant/covenants/src/templates.rs), [constructor defaults](../../../../finstack-quant/covenants/src/engine/types.rs), and [execution semantics](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs). Inspected `templates_accept_valid_thresholds`, invalid-input tests, kind-tagged strict serde tests, and [engine convention test](../../../../finstack-quant/covenants/tests/engine_conventions.rs) `project_finance_lockup_breach_blocks_distributions_without_default`.

No template is adopted by this documentation.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
