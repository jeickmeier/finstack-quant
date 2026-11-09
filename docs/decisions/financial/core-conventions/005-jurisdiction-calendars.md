# CORE-CONVENTIONS-005: Jurisdiction-specific substitutions and maintained holiday selections

- Decision status: **Accepted — complete jurisdiction-calendar package, 2026-11-07**.
- Implementation status: **Partly reconciled 2026-11-08**: the generic `cme` and `cnbe` IDs are retired and commodity forwards take their contract's settlement calendar (rejecting `nerc` for settlement); the remaining calendar corrections, annual-notice data and metadata purposes are pending, scoped to `dates/calendar/jurisdictions.rs` and all 28 audited `core/data/calendars/*.json` definitions.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent date-example checks passed; production calendar tests, data generators and fixture refreshes were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The implementation distinguishes banking, fixing, accrual and exchange calendars by ID. Rule selections are contractual data, not interchangeable labels. The complete inspected rule parameters, including year gates, are reproduced in [the calendar selection appendix](calendar-selections.md); its source links resolve to all 28 data files. Those definitions are an observed snapshot, not a claim that every year is externally verified.

## Accepted decision: jurisdiction calendar purposes and coverage

Accepted on **2026-11-07** after whole-record review. This complete package adopts purpose, legal-rule
selection, coverage/forecast eligibility and required corrections together.
Runtime reconciliation and independent qualification remain open. Apply
[004's accepted engine/data boundary](004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries)
without reopening its rule, weekend, error, caching or input-admission choices.
The 28 audited JSON files and their old parameters remain source evidence;
the accepted package selects 26 purpose-scoped IDs and retirement of two ambiguous IDs.
It does not certify the current rows or execute calendar reconciliation.

1. **Make calendar purpose part of the financial contract.** Banking,
   payment settlement, reference-rate value dates, cash trading, common
   exchange closures, public holidays and power holiday labels are distinct
   uses. Select an ID explicitly from the contract/service requirements;
   do not infer one from currency or silently substitute a nearby country's,
   exchange's or banking calendar. Identical dates in one year do not make
   two purposes interchangeable. Keep intentionally separate IDs with shared
   implementation where their contractual purposes genuinely differ. A
   valid registry lookup does not establish fitness for a particular use.

2. **Adopt the following complete purpose and forecast matrix.** The
   forecast permission below applies only to a complete, qualified model
   within its declared technical/legal range; it does not activate an
   unqualified dataset. Published schedules and dated exceptions in the
   selected frozen source edition take precedence. Historical use requires
   explicit supported coverage; a future-rule model is not permission to
   backfill unknown historical exceptions.

   | Audited ID(s) | Selected purpose / action | Coverage and forecast contract |
   | --- | --- | --- |
   | `asx` | ASX cash-market full-day trading/CHESS date convention; early sessions stay eligible. | Reconcile published service-specific schedules; permit tagged current-rule forecasts, not all ASX products or session hours. |
   | `auce` | Sydney/NSW weekday banking/business-centre convention, including the August bank holiday. | Correct dated NSW additions below. Permit tagged current-law recurrence; do not invent a permanent Anzac substitution rule. |
   | `brbd` | ANBIMA national business-day accrual convention. | Keep national dates, including November 20 from 2024. Permit source-qualified calculated future dates as forecasts; municipal, B3 trading and retail/year-end closure purposes are excluded. |
   | `nse`, `bse` | Separate weekday cash-market closure conventions. | Use each exchange's complete annual circulars plus amendments. Current moving rows cover 2024–2026 but have exceptional omissions; reconcile before declaring complete coverage. Do not infer actual Saturday, special or Muhurat-session availability. No incomplete religious-date forecast is adopted. |
   | `cato` | Toronto weekday banking/business-centre convention. | Retain the explicit local convention, including Family/Civic Day; qualify its source and era. Permit tagged recurring forecasts. It is not Payments Canada/Lynx, TSX or an all-Canada calendar. |
   | `tsx` | TSX cash-equity trading closures. | Published TMX dates and exceptions govern; permit tagged recurring forecasts. Trading eligibility does not determine settlement eligibility. |
   | `chzh` | CHF/Zurich weekday bank-centre convention. | Qualify the exact centre/service source edition; permit tagged recurrence. Do not treat it as all Swiss bank branches, euroSIC or SIX trading. |
   | `six` | SIX Swiss Exchange full-day trading closures. | Retain December 24/31 closures where published; permit tagged recurring forecasts. Settlement/currency calendars remain separate. |
   | `cme` | Retire the generic eligible financial-calendar ID. | CME schedules are product/platform-specific; remove the unsupported group-wide full-day claim and ID, without a fallback or alias. |
   | `cnbe` | Retire the generic mainland banking ID. | A government/SSE-style closure model does not identify a banking/payment service or represent required working weekends. Remove the unsupported ID; do not silently map it to `sse`. |
   | `defr` | Correct to the Frankfurt/Hesse weekday bank-centre convention. | Include the Hesse/Bundesbank distinctions below; qualify the era/source. Permit tagged recurrence. Stop duplicating TARGET under a Frankfurt label. |
   | `eurex` | Common exchange-wide full trading closures. | Use published Eurex regulations and permit tagged recurrence; product/currency-specific restrictions are additional. This is not a clearing or complete per-product calendar. |
   | `gblo` | England/Wales bank-holiday dates for London. | Include dated moved/extra holidays. Permit tagged recurrence; do not infer Scotland, Northern Ireland or LSE sessions. |
   | `hkhk` | Hong Kong Monday–Friday banking-centre dates excluding Cap. 149 general holidays. | Apply the legal epochs and bounded history below. Permit complete, qualified lunar/current-law forecasts with retained forecast status; no claim about every clearing service or labour-law statutory entitlement. |
   | `hkex` | HKEX cash-securities full-day date eligibility. | Apply securities-specific published notices and supported-era exceptions. Permit tagged scheduled-date forecasts; do not infer derivatives, half-day hours or actual weather/session availability. |
   | `jpto` | Japan banking full-day date convention: national holidays plus January 2/3 and December 31. | Use epoch-correct laws and supported banking-era coverage. Permit qualified current-law/equinox forecasts only while all required inputs exist. |
   | `jpx` | Japan cash-market full-day date convention. | Use cash-market notices, law and supported-era coverage. Permit qualified scheduled-date forecasts; do not infer derivatives holiday trading or historical Saturday sessions. |
   | `nerc` | Six additional off-peak holiday labels only. | Retain the named recurring holiday convention and Sunday substitution, without adding Juneteenth. This permission is for labels; generic weekday counts are not power peak/off-peak hours. |
   | `nyse` | NYSE cash-equity full-day trading closures. | Preserve the Saturday-New-Year distinction and dated exceptional closures. Permit tagged current-rule forecasts; no blanket historical, options or session qualification. |
   | `nzau` | Auckland regional weekday public-holiday convention. | Use Auckland notices, dated Mondayization epochs and legislated Matariki dates through 2052. Permit a complete qualified recurring model within that data horizon; later missing Matariki data error. Not NZX or Wellington. |
   | `sgsi` | Singapore civil public holidays. | Use complete maintained Ministry of Manpower annual schedules, currently 2024–2027. Other years requiring absent moving dates error; no Gregorian-only fallback. Not SGX or a payment-service calendar. |
   | `sifma` | Published SIFMA U.S. bond-market full-day trading recommendations. | Use complete pinned annual schedules/amendments. Early-close dates remain eligible. No blanket future Good-Friday/full-close model is adopted. Not a settlement mandate, SOFR calendar or MBS settlement-date table. |
   | `sofr` | Treasury-repo/reference-rate value-date business-day convention. | Preserve NY Fed repo-market notices and eligible tagged recurring forecasts, with explicit source/exception coverage from the benchmark's actual history. Publication timing and value/publication-date mapping remain separate. |
   | `sse` | Shanghai cash-market full-day trading closures. | Use complete pinned SSE annual notices and amendments only. Remove automatic ChinaBridge market selection and pre/post-window estimates. No complete discretionary-holiday forecast model is adopted. |
   | `target2` | T2 euro cash-settlement closing dates. | Retain the named settlement purpose despite the legacy ID spelling. Permit tagged current-rule forecasts; do not infer TIPS, all T2S operations or ECB staff holidays. |
   | `usny` | Federal Reserve weekday banking-date convention. | Retain Sunday-only substitution and the applicable statutory epochs; permit qualified recurrence. Not OPM/NYSE observance, FedNow or service-specific session availability. |
   | `weekends_only` | Explicit Saturday/Sunday-only convention. | No jurisdictional holidays or implicit country substitution; use the supported `Date` domain. |

   The [existing appendix](calendar-selections.md) remains the exhaustive
   parameter snapshot for the audited inputs, including selections that
   require correction or retirement under this package.

3. **Correct the known regional/purpose mismatches explicitly.** Add
   `2026-04-27` and `2027-04-26` to `auce` under the
   [NSW published dates](https://www.nsw.gov.au/about-nsw/public-holidays)
   and two-year Anzac trial; do not add those dates to the separate ASX cash
   convention or extrapolate the trial indefinitely. For `defr`, select
   Frankfurt/Hesse bank-centre dates, adding Ascension, Whit Monday, Corpus
   Christi, October 3 and December 24/31 to the current six TARGET dates.
   The [Bundesbank 2027 publication](https://www.bundesbank.de/resource/blob/764518/84d26dc9e3e0cc1e6e3042290df060ff/472B63F073F071307366337C94F8C870/feiertage-in-deutschland-1-data.pdf)
   distinguishes national, regional and service-specific closures; do not
   infer those dates for T2 or every payment system. Preserve the local
   `cato`/`tsx` distinction: [Payments Canada](https://www.payments.ca/system-closure-schedule)
   omits Family/Civic Day from its systems' closures, while TSX trading
   differs on September 30/November 11. Qualify `chzh` against its selected
   CHF/Zurich convention rather than inferring universal Swiss service dates.

4. **Keep U.S. banking, cash trading, bond recommendations and repo dates
   separate.** Use [Fed K.8](https://www.federalreserve.gov/aboutthefed/k8.htm)
   for the selected `usny` convention; a Saturday holiday does not create
   a preceding-Friday Federal Reserve closure. Preserve NYSE's own rules,
   including Saturday New Year and the announced `2025-01-09` closure.
   Reconcile SIFMA from its [annual archive](https://www.sifma.org/resources/guides-playbooks/us-holiday-archive),
   removing unsupported nearest-Friday New-Year/Veterans closures and
   treating Good Friday 2021, 2023 and 2026 as early-session dates, not full
   closures. December 31, 2021 is early-close-only. Annual publication
   controls Good Friday status; no generic observed-holiday rule overrides
   it. Preserve SOFR's repo-specific Good Friday and announced July 2026
   closures: [NY Fed April notice](https://www.newyorkfed.org/markets/opolicy/operating_policy_260312a)
   and [July notice](https://www.newyorkfed.org/markets/opolicy/operating_policy_260618a).
   A repo value date is not itself a publication timestamp or an instruction
   to use a SIFMA/Fed calendar for every benchmark.

5. **Use complete annual notices where the model lacks a qualified full
   calendar.** For SSE, freeze the [2026 annual notice](https://www.sse.com.cn/disclosure/announcement/general/c/c_20251222_10802507.shtml)
   and its amendments as a complete source-backed candidate year; older
   Spring/Labour spans alone do not establish complete 2004–2026 coverage.
   Reconcile all annual holidays, including the known missing
   [October 8, 2020 closure](https://www.sse.com.cn/disclosure/announcement/general/c/c_20191220_4969627.shtml),
   before admitting that historical year. Mainland government
   make-up weekends do not become SSE trading days. For NSE/BSE, reconcile
   independent exchange annual circulars plus later amendments, including
   the relevant January 22, May 20 and November 20, 2024 and January 15,
   2026 notices; published source evidence includes
   [NSE May 2024](https://nsearchives.nseindia.com/content/circulars/CMTR61518.pdf),
   [November 2024](https://nsearchives.nseindia.com/content/circulars/CMTR64960.pdf)
   and [January 2026](https://nsearchives.nseindia.com/content/circulars/CMTR72260.pdf).
   Do not copy one exchange's schedule to the other without evidence.
   [NSE's January 20, 2024 regular Saturday session](https://nsearchives.nseindia.com/content/circulars/MSD60340.pdf)
   demonstrates why these weekday-only conventions cannot promise complete
   actual trading availability. Purposes requiring Saturday openings or
   special-session timing remain unsupported by this engine. Preserve
   Singapore's bounded complete moving-date schedules and fail when required
   dates are absent; retain the legislated Auckland Matariki horizon rather
   than guessing dates beyond 2052.

6. **Adopt Hong Kong law by epoch, without inventing historical coverage.**
   Keep the selected Cap. 149 general-holiday dates and their modern Sunday
   substitutions. Saturday holidays do not automatically create Friday or
   Monday substitutes. In the applicable collision regime reserve ordinary
   holidays first and select the next day that is neither Sunday nor an
   already reserved general holiday; Saturday is not excluded by that legal
   search even though this engine's business-day predicate excludes it.
   Preserve the [2012 commencement](https://www.labour.gov.hk/eng/news/Substitution_of_Holidays.htm):
   the earlier Lunar New Year Sunday rule applies through New Year 2012,
   while the earlier Mid-Autumn rule ends with 2011. Modern collision logic
   is supported by the [government explanation](https://www.info.gov.hk/gia/general/201105/04/P201105040111.htm);
   pre-1999 historical use needs the actual ordinances/notices rather than
   blanket extrapolation of the current collision loop. The extra April 7,
   2026 closure from Qing Ming/Easter overlap is preserved. General holidays
   are not the separate labour-law statutory-holiday entitlement set.
   Keep `hkhk` and `hkex` purposes distinct despite shared national-date code.

7. **Adopt Japan's epoch-specific national/substitute/citizens rules.**
   Build the national set from the applicable law and dated exceptions,
   retaining the Happy-Monday changes, emperor/accession changes and exact
   Olympic 2020/2021 moves. Sunday substitutes begin on April 12, 1973.
   Before 2007 the next day is the substitute; from 2007 search past national
   holidays only, not the complete bank/substitute/citizens closure set.
   Citizens holidays are derived from two surrounding national holidays,
   not recursively from every closure. For 1986–2006 apply the original
   Sunday/substitute exclusions; from 2007 apply the amended non-national-day
   rule without preserving an obsolete extra Sunday exclusion. Sources:
   [Cabinet Office](https://www8.cao.go.jp/chosei/shukujitsu/gaiyou.html),
   [1985 amendment](https://www.shugiin.go.jp/internet/itdb_housei.nsf/html/houritsu/10319851227103.htm)
   and [2005 amendment](https://www.shugiin.go.jp/internet/itdb_housei.nsf/html/housei/16220050520043.htm).
   Add the selected banking/cash year-end closures separately; they must not
   manufacture additional national substitute holidays. Preserve published
   equinoxes versus qualified forecasts under 004.

8. **Bound historical and product eligibility by what this engine can
   represent.** The Japan 1970–2027 national fixture does not certify
   historical banking/trading regimes: Saturday operations and different
   year-end closures existed before February 1989. Pre-1970 national law
   also needs its original adoption gates. Likewise, HK clearing used
   Saturdays before September 2006. Require actual era/service evidence
   before admitting those financial purposes; do not reinterpret them as
   today's Monday–Friday convention. `jpx`/`hkex` are cash-date purposes:
   [JPX derivatives holiday trading](https://www.jpx.co.jp/english/derivatives/rules/holidaytrading/index.html)
   and [HKEX derivatives holiday trading](https://www.hkex.com.hk/Services/Trading/Derivatives/Overview/Trading-Mechanism/Derivatives-Holiday-Trading?sc_lang=en)
   are distinct. Half-days can remain eligible dates without this engine
   defining hours. `nerc` supplies six holiday labels; its ordinary weekday
   counter must never be used as a product's hourly peak profile. That
   product owns weekdays, hours, time zone and DST; see the
   [ISO-NE operating procedure](https://www.iso-ne.com/static-assets/documents/2015/12/crop_31001.pdf).

9. **Make forecasts deliberate, complete and reproducible.** The matrix
   explicitly permits complete current-rule forecasts for the named purposes
   within their declared domains. Preserve their forecast status until the
   selected edition contains actual publication evidence. Do not use current
   wall-clock time to relabel a frozen dataset. New official schedules or
   amended notices override model dates in a reviewed new input revision;
   never silently change a valuation's frozen calendar mid-run. There is no
   automatic projection for SSE/SIFMA annual-notice gaps, missing NSE/BSE or
   Singapore moving dates, Matariki after 2052, missing lunar tables or
   unsupported historical regimes. A forecast label does not qualify an
   incomplete model or excuse omission of already known notices. Outside
   supported data/domain, return 004's explicit error rather than another
   calendar, a Gregorian-only subset or an estimated holiday set. No generic
   projection switch, new registry or session/weekend-reopening framework
   is adopted here.

10. **Qualify the full calendar, including open dates, before advertising
    coverage.** Keep a compact source/era/purpose/coverage record with each
    retained definition, distinguishing complete published-year fixtures,
    supported recurring models, forecasts and still-unqualified observations.
    Reconcile every listed closure and every unlisted weekday for admitted
    annual schedules; include spillover boundaries, one-off notices, known
    open neighbors and the full required source years. Spot assertions and
    agreement with the old implementation digest are insufficient. Preserve
    the audited appendix as historical source evidence; corrections and
    qualification are owned by the [existing follow-ups](../FOLLOW_UPS.md#core-conventions).
    Remove retired IDs/inputs/exports rather than retaining compatibility
    aliases; [CME's product/platform schedules](https://www.cmegroup.com/trading-hours.html)
    illustrate the unsupported generic-ID claim. Reconcile Rust, generated
    data, metadata, Python/WASM, wire/parity surfaces and consuming examples
    together. This is methodology review: runtime changes, data refreshes
    and independent calendar qualification remain pending.

## Boundary examples for the accepted package

These source-backed cases define expected behavior after reconciliation;
they are not claims that production tests were executed in this review.

| Case | Required outcome |
| --- | --- |
| NSW `auce` versus ASX on `2026-04-27` / `2027-04-26` | Dated NSW addition; do not infer the same ASX closure or a permanent future Anzac rule. |
| Frankfurt/Hesse on Ascension / Corpus Christi | Closed under the selected bank-centre convention, with no corresponding TARGET-wide inference. |
| `2026-04-03` Good Friday | NYSE closed, SIFMA early session remains an eligible date, `usny` open and SOFR repo date closed. |
| `2021-12-31` under SIFMA | Early-close-only date; do not manufacture a full closure from Saturday New Year. |
| Toronto Family/Civic Day; September 30 / November 11 | Preserve the `cato`/`tsx`/Payments Canada purpose distinctions rather than treating their open-day sets as equal. |
| Hong Kong `2026-04-07` | Preserve the additional general-holiday closure from the Qing Ming/Easter overlap. |
| Japan `2026-09-22` | Citizens holiday between the September 21/23 national holidays. |
| SSE 2026 Mid-Autumn and National Day | Separate September 25–27 and October 1–7 blocks; September 28 and October 8 reopen under the annual notice. |
| SSE `2020-10-08` | Reconcile the known omitted full closure before admitting complete 2020 coverage. |
| NSE `2026-01-15` | Reconcile the dated election closure; annual recurring festivals alone are incomplete. |
| An unmaintained SG/Indian moving-date year, SSE annual-notice gap or Auckland after 2052 | Explicit coverage error, without incomplete fallback. |
| `cme` or `cnbe` after the accepted retirement is implemented | Unknown/unsupported ID error, without aliasing another calendar. |

## Observed algorithm: Hong Kong

`hkhk` and `hkex` use the same jurisdiction function. Start with January 1 and December 25/26, Sunday-shifted; three Chinese New Year days; Qing Ming, Dragon Boat, Chung Yeung; Good Friday, Easter Saturday and Easter Monday; and the day after Mid-Autumn. A Sunday within the Chinese New Year block substitutes the day before New Year for 1983–2012, otherwise day four. The day after Mid-Autumn falling Sunday uses festival day in 1983–2011, otherwise Monday.

Add May 1/Buddha's Birthday from 1999, July 1/October 1 from 1997. Historical branches include April 21/July 1/first August Monday through 1982, second June Saturday plus following Monday through 1997 (third Saturday in 1984), last August Monday through 1996 plus preceding Saturday from 1983, August third Monday/October 2 in 1997–98, July 2 in 1997, and dated 1981/1986/2015 closures. Sunday-shift every resulting date, sort, deduplicate, and for each duplicate collision append the next date not Sunday and not already a holiday. Saturdays are not skipped by that collision algorithm, though the business-day predicate separately excludes them.

## Observed algorithm: Japan

`jpto` and `jpx` combine `japan_public_holidays` with January 2/3 and December 31. The national function includes Jan 1, Feb 11, Apr 29, May 3/5, Nov 3/23 and tabulated equinoxes. Coming-of-Age moves from Jan 15 to second January Monday in 2000; Respect-for-Aged from Sept 15 to third September Monday in 2003; Marine Day starts in 1996 and changes July 20 to third July Monday in 2003; Sports Day changes Oct 10 to second October Monday in 2000. Olympic 2020/2021 branches replace Marine/Sports/Mountain dates. Mountain Day begins in 2016; May 4 becomes explicit from 2007; Emperor's Birthday is Dec 23 for 1989–2018 and Feb 23 from 2020, with dated accession/wedding/funeral closures.

From 1986, insert a non-Sunday day sandwiched between two national holidays. For Sunday national holidays on/after 1973 day 102, add Monday; from 2007 skip over other national holidays before selecting the substitute. Sort and deduplicate. The skip set is the national dates, not the entire expanded substitute/citizens set.

## Observed selection, conventions and controls

The appendix records every selected rule. Material distinctions include `usny` Sunday-only federal substitution; `nyse` Saturday-New-Year exception and 2025 mourning closure; `sofr` repo closures even when `sifma` has an early trading session; `cato` versus `tsx` September/November closures; `chzh` versus `six` and `target2`/`defr` versus `eurex` December 24/31 closures. `brbd` selects national accrual holidays rather than B3 trading closures. `nerc` supplies six off-peak holiday labels; its generic business-day predicate still excludes Saturdays/Sundays and is not a power hourly peak-shape calculator.

## Observed limitations, failure and fallback

Calendar definitions explicitly limit moving Indian exchange festivals to 2024–2026, Singapore moving holidays to 2024–2027, and Matariki dates to 2022–2052. China Spring Festival/Labour Day dated spans cover 2004–2026; outside that range named estimates are used, and single-day bridge rules approximate historical arrangements. There is no runtime completeness error inside those uncovered windows. All calendars use a full-day model. Hong Kong/JP historical algorithms extend beyond independently stored fixture years. Upcoming exceptional holidays require data maintenance.

## Alternatives and tradeoffs

The accepted matrix preserves separate banking, reference-date, accrual and trading purposes, and retires generic CME/mainland-banking IDs. Exact published annual tables require maintained notices and amendments. Complete qualified recurring forecasts retain their projected status and cannot substitute for missing annual schedules where the matrix requires them. Unknown historical exceptions do not become supported coverage. Session-aware opening schedules and universal cross-market defaults remain outside the selected contract.

## Evidence

[Jurisdiction code](../../../../finstack-quant/core/src/dates/calendar/jurisdictions.rs), [published annual-calendar tests](../../../../finstack-quant/core/tests/dates/calendars.rs), [published weekday closures](../../../../finstack-quant/core/tests/dates/fixtures/published_weekday_closures.csv), and [Japan source fixture](../../../../finstack-quant/core/tests/dates/fixtures/japan_public_holidays.csv). The inspected test `calendars_match_complete_published_years_including_open_days` checks every weekday in listed years, including unlisted open days; `japan_public_holidays_match_cabinet_office_1970_through_2027` compares a stored Cabinet Office snapshot. The fixture identifies source URLs and access date; this audit did not refresh them. [Fed K.8](../../../REFERENCES.md#fed-k8-holidays) is the central banking-calendar reference.

Calendar-specific provenance references live mainly in data/fixture comments; central reference coverage is incomplete.

The national-date fixtures and inspected publications have narrower evidence
roles than complete historical financial-calendar coverage. The accepted
package above identifies required purpose, input and statutory-epoch corrections;
source inspection does not establish their runtime implementation.
Selected official sources were consulted for the 2026-11-07 decision review; stored
fixtures were not refreshed or executed, and the source-audit metadata above
retains its original date and revision.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
