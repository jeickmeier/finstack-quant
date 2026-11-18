# COVENANTS-11: Deal-package assembly and embedded consequence assumptions

**Decision status:** Accepted 2026-11-17 for named package assembly with explicit agreement terms.

**Implementation status:** Four named package builders are implemented. Explicit operative inputs, shared validation and the project-finance DSCR tier check remain pending.

**Validation status:** All three template tests and the project-finance lockup regression passed on 2026-11-17 against current behavior. Explicit terms and the accepted DSCR ordering remain to be qualified.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Keep LBO, cov-lite, real-estate and project-finance packages as assembly helpers. Require agreement-supplied thresholds, metric and denominator bindings, labels, scope, activation, cadence, cure choices and consequences, including an explicit choice of no consequence. Use the same Rust validation as individually assembled specs. Keep applicable cov-lite restrictions visible as requiring external assessment. [Explicit terms](01-terms-and-metric-selection.md) · [Assessment scope](04-activation-and-scope.md).
2. In the named project-finance package, use a shared DSCR definition for the default and lockup tests and require the lockup floor to be strictly higher. With floors of 1.30× and 1.20× respectively, 1.25× breaches only lockup. Use individual specifications when definitions or tier structures differ. Total versus senior leverage threshold ordering follows the agreement.
3. Require an explicit agreement rule for competing sweeps or other actions. Execution depends on typed target inputs and the [accepted consequence contract](06-consequence-execution.md).

## Observed package construction

`covenant_template` dispatches on four tagged variants and validates each input only as finite and nonnegative. Ratios are turns; debt yield and LTV are decimal fractions without an upper bound; capex/liquidity are untyped reporting-currency amounts. The constructors return ordered specs and do not infer a deal's governing documents.

| Package | Ordered tests and embedded policy |
| --- | --- |
| LBO standard | Quarterly maintenance debt/EBITDA (`max_debt_ebitda`, 30-day cure, +200bp rate); interest coverage (`min_interest_coverage`, 30 days, distribution block); fixed-charge coverage (`min_fcc`, 30 days, no consequence); annual capex (`max_capex`, inherited 30 days, no consequence). Leverage is constant unless the caller later attaches a schedule. |
| Cov-lite | Quarterly incurrence total and senior leverage (`max_total_leverage`, `max_senior_leverage`); annual descriptive negative covenant (`negative`) against additional secured debt without consent. All inherit 30-day cure and no consequences. The negative test is not numerically assessed. |
| Real estate | Quarterly maintenance DSCR (`min_dscr`, 30 days, 100% sweep); custom minimum debt yield (`min_debt_yield`, inherited 30 days, no consequence); custom maximum LTV (`max_ltv`, inherited 30 days, 50% sweep). |
| Project finance | Quarterly maintenance DSCR default (`min_dscr_default`, 60 days, Default); DSCR lockup (`min_dscr_lockup`, inherited 30 days, distribution block); liquidity (`min_liquidity`, inherited 30 days, no consequence); net debt/EBITDA (`max_net_debt_ebitda`, inherited 30 days, default denominator `ebitda`, no consequence). |

Frequency remains metadata; the engine will test annual capex on any caller date without constructing annual expenditure. Labels distinguish two DSCR tests and two custom CRE tests, but concatenating packages with duplicate labels can fail engine validation. Independently valid template input does not establish a consistent joint capital structure, reserve, or earnings convention.

## Observed contract difference and limitations

Project-finance documentation calls lockup DSCR the higher threshold. The current builder validates thresholds individually and its valid-template test accepts default DSCR 1.2 with lockup DSCR 1.1. Shared-measure ordering under the accepted package contract remains to be enforced. The current builder has no cross-input ordering check for total and senior leverage.

CRE sweeps are separate setter actions in caller/history order, not a maximum-of-triggered-sweeps rule. Project default and CRE sweeps cannot simply be executed through the inspected TermLoan adapter without extra recovery/cash-flow inputs; see [COVENANTS-06](06-consequence-execution.md). A named package is not a market/legal standard certification. In particular, source phrases such as “Post-2015 ... standard” have no contract sample or empirical provenance in the inspected module.

## Rationale, alternatives and evidence

Named assembly retains the package's test structure and keeps the agreement in control of operative terms. Distinct instance keys preserve separate lockup and default outcomes and their captured consequences.

Owning [template definitions/builders/tests](../../../../finstack-quant/covenants/src/templates.rs), [constructor defaults](../../../../finstack-quant/covenants/src/engine/types.rs), and [execution semantics](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs). Inspected `templates_accept_valid_thresholds`, invalid-input tests, kind-tagged strict serde tests, and [engine convention test](../../../../finstack-quant/covenants/tests/engine_conventions.rs) `project_finance_lockup_breach_blocks_distributions_without_default`.

The current tests passed. Qualify explicit required inputs, package expansion parity with individually assembled specs, equal and inverted DSCR tiers, and agreement rules for competing sweeps.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
