# `data/margin` — regulatory and market-convention reference data

Six JSON files, embedded into the `finstack-quant-margin` binary at build time
by `src/registry/embedded.rs` (`include_str!`), parsed by `src/registry/mod.rs`,
and overlayable at runtime through the Finstack config extension key
`margin.registry.v1`.

This README exists because these files carry numbers that move capital and
collateral amounts, and until 2026-08-20 none of them recorded where its
numbers came from.

**Read this before editing any file here.** Changing a value here changes
computed margin. Every edit needs a source citation and a reviewer.

---

## Provenance status at a glance

| File | Source | Version / as-of | Test-pinned? |
|------|--------|-----------------|--------------|
| `simm.v1.json` | ISDA SIMM Methodology | v2.6 (Dec 2023) and v2.5 (Dec 2022) | **Yes** — partial, `tests/simm_schedule_parity.rs` |
| `schedule_im.v1.json` | BCBS-IOSCO standardised IM schedule | BCBS 261 / d499 Appendix A | Partial — 6 of 18 rate cells |
| `collateral_schedules.v1.json` | BCBS-IOSCO standardised haircut schedule + market convention | BCBS 261 / d499 Appendix B | Partial |
| `defaults.v1.json` | Market convention (ISDA CSA practice), **not** a published schedule | Convention, as-of 2026-08-20 | Partial |
| `ccp_methodologies.v1.json` | **Heuristics.** No published source. | Convention, as-of 2026-08-20 | Partial |

"Test-pinned" means a test asserts the literal value, so an accidental edit
fails CI rather than silently changing a margin number.

---

## `simm.v1.json`

**Source.** International Swaps and Derivatives Association, *ISDA SIMM
Methodology*. One historical version is embedded:

| Registry id | ISDA document | Frozen on |
|-------------|---------------|-----------|
| `v2_6` | ISDA SIMM Methodology Version 2.6 (December 2023) | 2024-01-15 |

Contains per-version MPOR, risk weights, correlations and concentration
thresholds across the SIMM risk classes. Concentration thresholds use USD;
inputs and output must be normalized to USD. The implementation is explicitly
approximate because some required factor and product-class dimensions are
not represented. Parameter provenance is not model certification.

**Pinned by.** `tests/simm_schedule_parity.rs`. That file is the authoritative
provenance record for this JSON: it carries the ISDA section/table reference
for each golden value and the full re-verification procedure for adopting a
new ISDA release. It pins a **subset** — enough to catch accidental edits and
loader drift, not every number in the file.

**Not pinned.** Any risk weight, correlation or threshold absent from
`SimmV26GoldenValues`.

**Regeneration.** Follow the numbered procedure in the module doc of
`tests/simm_schedule_parity.rs`. ISDA publishes annually; the embedded copy
must be re-verified against the ISDA PDF on every schedule update.

---

## `schedule_im.v1.json`

**Source.** Basel Committee on Banking Supervision and Board of the
International Organization of Securities Commissions, *Margin requirements for
non-centrally cleared derivatives* — the standardised initial margin schedule
(originally BCBS 261, September 2013; consolidated in BCBS **d499**, revised
April 2020), Appendix A.

**Registry id.** `bcbs_iosco`.

**Contents.** The 18 `(asset_class, bucket)` initial-margin rates, plus the
bucket boundaries (2 and 5 years) and a 10-day MPOR:

| Asset class | 0-2y | 2-5y | 5y+ |
|-------------|------|------|-----|
| Interest rate | 1% | 2% | 4% |
| Credit | 2% | 5% | 10% |
| Equity | 15% | 15% | 15% |
| Commodity | 15% | 15% | 15% |
| FX | 6% | 6% | 6% |
| Other | 15% | 15% | 15% |

All 18 values match Appendix A as published (verified 2026-08-20).

**Pinned by.** `src/calculators/im/schedule.rs` tests. `bcbs_schedule_rates`
asserts the three interest-rate buckets (1%, 2%, 4%), the credit short and
long buckets (2%, 10%) and the equity flat rate (15%) — 6 of the 18 rate
cells, and implicitly the 2y/5y bucket boundaries via the maturity arguments.
`schedule_im_calculation` and `credit_schedule_im` re-assert the
interest-rate long (4%) and credit long (10%) rates through the calculator.

**Not pinned.** The credit medium bucket (5%), all commodity, FX and Other
rates, the `default_rate` of 15%, `default_asset_class`,
`default_maturity_years`, and `mpor_days`.

**Regeneration.** Re-read Appendix A of the current BCBS-IOSCO consolidated
text, update the `rates` array, and extend the tests above to cover any value
you change.

---

## `collateral_schedules.v1.json`

**Source.** Two distinct origins, mixed in one file — be careful which you are
editing.

1. `asset_class_defaults` and the `bcbs_standard` entry: the BCBS-IOSCO
   standardised haircut schedule (BCBS **d499**, Appendix B) — cash 0%,
   government bonds 2%, corporate bonds 5%, equity 15%, gold 15%, and the
   **8% additional haircut for a currency mismatch** between the collateral
   and the settlement currency (`fx_addon`).
2. The `cash_only` entry and the `concentration_limit` fields: **library
   convention**, not published values. `concentration_limit: 0.30` on
   corporate bonds in particular has no BCBS source.

**Registry ids.** `cash_only`, `bcbs_standard`.

**Pinned by.** `src/types/collateral.rs::total_haircut_includes_fx_addon`
pins the government-bond `fx_addon` of 8%, which it reads from this file via
`CollateralAssetClass::GovernmentBonds.defaults_or_panic()`. Note the 2% in
that test is a caller-supplied argument, **not** the file's
`standard_haircut` — so the 2% government-bond haircut in this file is not
actually pinned by anything.

**Not pinned.** Every `standard_haircut` value, the seven other `fx_addon`
entries, all `concentration_limit` values, and `rehypothecation_allowed`.

**Regeneration.** For the BCBS-sourced values, re-read Appendix B of the
current consolidated text. The convention-sourced values need a desk decision,
not a document.

---

## `defaults.v1.json`

**Source.** **Market convention, not a published schedule.** These are the
defaults `CsaSpec::usd_regulatory()` and `OtcMarginSpec::usd_bilateral()`
resolve to when a caller supplies no explicit terms.

Where the numbers come from:

- `vm.mta = 500_000` and `im.*.threshold = 50_000_000` are the **regulatory
  maxima** in the BCBS-IOSCO framework (the EUR 50m IM threshold and EUR 500k
  minimum transfer amount, expressed here in the reporting currency). These
  are ceilings a real CSA may sit below, not prescribed values.
- `vm.rounding = 10_000`, `settlement_lag = 1`, and every `timing.*` value are
  ISDA CSA drafting practice, not published requirements.
- `im.simm.mpor_days = 10` and `im.schedule.mpor_days = 10` match the
  BCBS-IOSCO 10-day margin period of risk for uncleared derivatives.
- `im.cleared.mpor_days = 5` and `im.repo_haircut.mpor_days = 2` are the usual
  cleared-derivative and repo conventions.

**As-of.** 2026-08-20. No expiry — these are conventions, and they change when
market practice changes.

**Pinned by.** `src/types/csa.rs::usd_regulatory_csa` pins only
`vm.threshold = 0`. `src/types/csa.rs::margin_call_timing_defaults` pins
`timing.standard.notification_deadline_hours = 13` and
`dispute_resolution_days = 2`.

**Not pinned.** `vm.mta`, `vm.rounding`, `settlement_lag`, every `im.*`
threshold and MPOR, the `regulatory_vm` and `ccp` timing blocks, and
`cleared_settlement`.

**Regeneration.** Desk decision. If you change a value here you change the
default terms of every CSA built without explicit parameters.

---

## `ccp_methodologies.v1.json`

**Source.** **None. These are heuristics.**

The `conservative_rate` per CCP (LCH SwapClear 2%, LCH CDSClear 8%, CME 3%,
ICE Clear Credit 10%, ICE Clear US 5%, JSCC 3%, Eurex 3%, generic 5%) is a
rough proxy for `IM / |exposure base|`. No CCP publishes such a rate, and none
of these figures is traceable to a CCP disclosure. They exist so
`ClearingImCalculator` can produce a number without a full CCP model.

The `generic_var` entry (99% confidence, 250-day lookback) reflects common CCP
VaR-model practice but is not any specific CCP's published parameterisation.
`mpor_days = 5` for every CCP matches the usual cleared-derivative margin
period of risk.

**Treat every output derived from this file as indicative, not as a CCP margin
estimate.** `src/calculators/im/clearing.rs` documents the same caveat.

**As-of.** 2026-08-20.

**Pinned by.** `src/calculators/im/clearing.rs::conservative_rates` (LCH
SwapClear 2%, ICE Clear Credit 10%), `::mpor_days` (LCH and CME, 5 days) and
`::conservative_calculation` (the LCH 2% rate through the calculator).

**Not pinned.** The LCH CDSClear, CME, ICE Clear US, JSCC, Eurex and generic
rates, and the whole `generic_var` parameterisation.

**Regeneration.** If a real CCP margin model or a published CPMI-IOSCO
quantitative disclosure becomes available, replace the heuristic and record
the source here.

---

## FRTB parameter review

The 146 FRTB standardised-approach risk weights and correlations are **not**
in this directory. They live as `pub const` tables in
`src/regulatory/frtb/params/`:

| Module | Covers |
|--------|--------|
| `params/girr.rs` | General interest rate risk |
| `params/csr.rs` | Credit spread risk — non-sec, securitisation CTP, securitisation non-CTP |
| `params/equity.rs` | Equity |
| `params/commodity.rs` | Commodity |
| `params/fx.rs` | Foreign exchange |
| `params/correlation_scenarios.rs` | The three MAR21.6 correlation scenarios |

**Source.** Basel Committee on Banking Supervision, *Minimum capital
requirements for market risk* (BCBS **d457**), published 14 January 2019,
corrected version 25 February 2019; consolidated as Basel Framework chapter
**MAR21**, version effective 1 January 2023 (text incorporates the FAQs
published 5 July 2024 and 23 March 2026).

Primary sources verified against on 2026-08-20:

- <https://www.bis.org/bcbs/publ/d457.pdf>
- <https://www.bis.org/baselframework/BaselFramework.pdf> (BIS's own PDF
  export of the consolidated framework; the HTML chapter view at
  `bis.org/basel_framework/chapter/MAR/21.htm` renders client-side and cannot
  be fetched non-interactively)

No numeric parameter changed between d457 and the consolidated text; the
consolidation made wording corrections only.

**Provenance records.** Each `params/*.rs` module carries its own provenance
table naming the exact paragraphs and tables it draws on, plus a **"Known
deviations from MAR21"** section where the implemented value differs from the
published one. `params/mod.rs` carries the cross-module deviation summary.

**Pinned by.** `params/mod.rs::tests` (20 tests covering the delta risk-weight
tables for all six risk classes, every correlation constant, the MAR21.92
vega liquidity-horizon formula, the MAR21.46 footnote-13 worked example, and
the lookup fallbacks) and `tests/frtb_sba_charges.rs` (11 end-to-end engine
charges with hand-written derivations).

**Review procedure.**

1. Fetch both primary sources above and confirm the MAR21 version header.
2. For the module you are touching, re-read every paragraph listed in its
   provenance table.
3. If a published value differs from the implemented one, **do not change it
   silently** — capital numbers move. Record it in that module's "Known
   deviations from MAR21" section, add or update the pinning test in
   `params/mod.rs::tests`, and route the change through a reviewer who can
   sign off on the capital impact.
4. Update the "Last reviewed" date in the module's provenance table and the
   verification date in this section.

Last full review: **2026-08-20**.
