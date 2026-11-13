# CORE-CONVENTIONS-019: Ordinal credit ratings and configurable scorecard scale data

- Decision status: **Accepted — complete ratings/scorecard package, 2026-11-07**.
- Implementation status: **Reconciled 2026-11-12** (`CreditRating` on 2026-11-09: ladder, partial order, fallible notches/Moody rendering, alias tables, hosts. Registry and scorecards on 2026-11-12: embedded `rating_scales.v2.json`, `embedded_registry`, default score/scale and `UnknownScalePolicy` removed; caller-supplied registry with nonblank source and strict normalized ids; private validated levels/scales/entries; `ScorecardScale::classify` with full `[0, 100]` coverage and representative self-membership; `CreditScorecardExtension::new(config, &registry)` snapshots the selected entry and rejects unknown or repeated threshold/minimum labels; Rust, Python and WASM).
- Validation status: **Evidence inspected; independent symbol/band examples and semantic review passed; production qualification pending**. Exact alias/rank/grade and illustrative score-boundary examples passed, and an independent calculation found 40 of 43 embedded representatives classify into another band. Linked Rust/host tests were located or inspected, not run; these checks do not certify current runtime/host behavior or agency calibration.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

This record owns generic rating symbols, comparison/classification and caller-defined
scorecard-scale state. Generic credit quality and numeric scorecard utility are
separate representations. Neither supplies PD, transition intensity, expected loss,
WARF, recovery, regulatory eligibility or proof of an assigned agency rating.
The complete package below replaces the observed rank/default/parser/data
choices without adopting the separately owned credit-model tables or scorecard
weighting/formula policy.

## Accepted decision: generic ratings and explicit scorecard scales

Accepted on **2026-11-07** after whole-record review. Runtime implementation
and independent production/host qualification remain pending;
observed source behavior is retained separately below.

1. **Retain one explicitly generic long-term quality shorthand.** Keep canonical
   labels AAA, AA+/AA/AA-, A+/A/A-, BBB+/BBB/BBB-, BB+/BB/BB-, B+/B/B-,
   CCC+/CCC/CCC-, CC, C, D and NR. The rated ladder is consecutive AAA=0 through
   C=20 and D=21; these are library notch positions, not equally spaced risks or
   numeric agency calibration. NR is explicit unrated status outside that ladder.
   Agency, issuer versus obligation scope, seniority, currency/local-national
   scale, observation date, outlook/watch and actual default events remain with
   the data/product owner. Do not add a new agency/source wrapper or ratings
   engine to this primitive, and do not infer those facts from a normalized label.

2. **Make NR unavailable for quality comparisons and notch arithmetic.** Remove
   total `Ord` and the numeric-discriminant ordering contract. Keep equality/hash
   identity and rated `PartialOrd`; rated pairs use AAA through D, NR versus rated returns
   incomparable, and NR versus itself is equal as a status identity, not equal
   assessed credit risk. `notches_to` returns Result and errors if either side is
   NR, including NR/NR; otherwise downgrade is positive, upgrade negative and
   equal rated labels zero. C->D is one library step, without an inserted NR
   notch. Do not sort NR into a financial floor or silently treat incomparable
   values as equal/worst/best. Reconcile rating-keyed ordered containers using
   existing deterministic maps/storage order, not a second misleading total
   financial comparator. Required minimum-grade thresholds cannot be NR.

3. **Keep grade predicates literal and nonexclusive.** Investment grade is
   AAA through BBB-; speculative grade is BB+ through D, so D is both speculative
   and the generic default marker. NR is neither. `is_default()` means exactly
   the selected D marker; false is not proof that an agency observation or obligor
   is performing. Moody C/Ca may already concern defaulted obligations; a generic
   symbol conversion cannot resolve that event/scope fact. `bucket()` only removes
   +/- within AA/A/BBB/BB/B/CCC; other values remain themselves. It is not a mean,
   midpoint estimate or probability/spread transformation.

4. **Replace permissive suffix stripping with finite alias tables.** Human
   parsing trims outer whitespace and is ASCII case-insensitive. Accept canonical
   generic tokens and actual Moody-style spellings: Aaa; Aa1/2/3; A1/2/3;
   Baa1/2/3; Ba1/2/3; B1/2/3; Caa1/2/3; Ca; C. The 1/2/3 families map to
   plus/flat/minus of their corresponding generic bucket; Ca maps CC. Keep only
   explicit project status aliases NR/NOTRATED/NOT RATED/UNRATED and D/DEFAULT.
   Do not remove arbitrary internal spaces, strip arbitrary suffixes or infer an
   agency from overlapping spellings: case-insensitive Aa matches generic AA,
   while bare Moody-only Baa/Ba/Caa are unsupported rather than invented middle
   notches. Reject AAA+, C3, D1, NR-, BBB2, CAA+, SD/RD, withdrawn/watch/outlook,
   short-term/national/structured modifiers and unknown text. Unsupported agency
   states need explicit owner treatment, not silent D/NR/coarse-grade conversion.
   This is a library symbol crosswalk, not agency equivalence or source provenance.

5. **Keep canonical interchange and honest style rendering.** Display and wire
   retain exact canonical generic strings; wire restoration accepts those labels
   only, not the human aliases or numeric ordinals. Do not default missing/invalid
   labels to NR. `to_moodys_string` becomes fallible: map supported quality
   shorthands to Aaa..C and keep NR only as explicit unrated status text, not an
   assigned agency opinion. D has no equivalent Moody long-term grade and errors;
   never fabricate Moody D or coerce D to C. The rendering does not reconstruct
   which agency supplied a value or establish its default event status.

6. **Treat scorecard numbers as an explicit internal utility policy.** Retain
   caller-defined ScorecardScale/RatingLevel and the existing registry, using
   finite scores/minima in inclusive [0,100], higher better. The numeric score is
   a chosen utility for its particular scorecard, not an ordinal notch, PD,
   expected loss, recovery, rating factor or equally spaced agency confidence.
   Named agency-style labels do not turn application-generated results into
   agency-assigned ratings. Policy authors own numerical spacing, calibration,
   source/vintage and fitness for their intended financial use; do not invent
   replacement universal numbers during this review.

7. **Remove unsupported embedded numeric defaults and fallback policy.** Retire
   the runtime embedded sp/moodys numeric dataset, `embedded_registry`, implicit
   sp selection, default score 50, default-scale accessors/fields and the entire
   UnknownScalePolicy choice. All lookups are strict; remove warning/fallback
   branches rather than retain an Error-only policy enum. Do not repair the
   current representative/threshold errors by silently changing assumed agency
   numbers. Missing metric/rating data is not automatically 0,50,worst or best.
   Any separately selected imputation belongs to its financial owner. Preserve
   the historical source audit below; deleting runtime defaults is a future
   implementation action, not an edit to the audited evidence or a data refresh.

8. **Require one explicit immutable registry and selected scale snapshot.**
   Keep direct registry construction/from_json and the existing configuration
   extension route; when a scale operation requests `core.rating_scales.v1`,
   absence/malformed data errors rather than falling back. Unrelated config
   users need not supply a registry. Require a nonempty validated registry and
   explicit scale ID; normalize IDs by outer trim/ASCII lowercase, reject blank/
   normalized duplicates, and perform no brand/fuzzy/punctuation alias inference.
   Actual scorecard construction resolves the selected entry once from that
   supplied registry and retains its immutable data/identity/source snapshot;
   it must not keep calling the embedded singleton or silently ignore configured
   replacement. Preserve complete caller-defined data plus nonblank source/policy
   provenance for replay; a brand name alone is not quantitative calibration.
   No mutable global override, new registry layer, auto-fetch or latest-edition
   restoration is introduced. Retain current schema/container identifiers in
   place under the existing development wire policy, without compatibility data
   or a migration framework.

9. **Define one complete score-to-band rule and validate its representatives.**
   Add one canonical `ScorecardScale::classify(score)` and reuse it in actual
   scorecard lookup and self-consistency validation. For best-to-worst rows with
   minima `m0>m1>...>m_last=0`, the best band is `[m0,100]`; each later band is
   `[mi,m_previous)`. Require finite input score in [0,100]; choose the first
   satisfied minimum, with no epsilon, clipping or last-row fallback. Thus the
   declared numeric domain is fully covered and each boundary belongs to the
   better band. Every representative must classify to its own row: at least its
   own minimum and, except the top, strictly below the preceding minimum. Keep
   strictly descending scores/minima and nonempty rows. A standalone level can
   check its lower bound; full-scale construction checks the preceding band.
   Do not replace representative scores with ordinal ranks or automatic midpoints.

10. **Normalize scale labels consistently and fail unknown references.** Trim
    level display names, require nonblank values and reject collisions after
    outer-trim/ASCII-case normalization within a scale; preserve the display case
    and do not collapse internal text/fuzzily reinterpret agency symbols. Reserve
    the explicit unrated aliases in point 4 as missing statuses, not numeric bands.
    Lookup of thresholds/minimum-rating labels uses that same selected scale/key
    normalization; duplicate normalized references and unknown labels error before
    evaluation rather than being ignored or returning false compliance. Minimum
    grade comparison uses positions in that exact scale, not generic CreditRating
    parsing/order or score differences. Numeric0 may classify to the worst utility
    band; this does not manufacture an actual agency default event or resolve
    unavailable evidence. Cross-scale label equality does not establish equal risk.

11. **Validate immutable state at every construction and restore boundary.**
    Make levels/scales/entries privately validated with read-only accessors and
    atomic complete constructors/restoration. Enforce finite domains, required
    identity/source, labels, order, full coverage and representative self-membership
    for all Rust/host/JSON/config paths; reject unknown/retired fields without
    sorting, renumbering, clipping, deduplication, imputation or default repair.
    An invalid proposed update leaves the previous value unchanged. Full registry
    snapshots preserve exact admitted model values, row order, IDs/source and scale
    definitions; restore validates them again. `to_table` preserves ordered
    name/score/min_score columns as a presentation view, not a complete policy
    snapshot or an agency/calibration certificate. Do not restore a lossy table
    as if it supplied the missing registry provenance.

12. **Keep scorecard economics and empirical credit data separately owned.**
    Metric formulas/weights, threshold interval overlap/gaps, boundary tie rules
    in metric inputs, missing-factor exclusions, weight renormalization, period
    choice and imputation are not adopted merely by validating the output scale.
    Reconcile those consumers to the selected snapshot and strict label checks
    without silently changing their separately owned methods. This package does
    not approve agency PD/transition/recovery/WARF data, regulatory mappings,
    rating-factor tables, NR collateral assumptions or jurisdiction-specific
    minimum-rating eligibility. Preserve actual agency/scope/default status and
    qualified policy inputs at those owners instead of inferring them from this
    generic shorthand or a scorecard result.

13. **Propagate the complete primitive/data/consumer surface together.** Update
    Rust re-exports, comparisons/notch/style APIs, key containers and raw casts,
    strict alias tables, registry/config/schema/data loading, classifier and private
    validated state, Python/WASM errors/stubs/facade/parity, examples/tests and
    actual scorecard/model/margin/coverage consumers in one working slice. Remove
    NR's numeric-rank fiction, suffix stripping, invalid Moody output, embedded
    numeric defaults, global/fallback accesses and stale calibration claims without
    aliases or migrations. Required financial policies outside this scope still
    need their owner decisions; acceptance does not implement or qualify them
    automatically. No new agency feed, rating engine or quantitative calibration
    framework is selected.

## Boundary examples for the accepted package

| Input / situation | Required accepted outcome |
| --- | --- |
| AAA..C,D versus NR | Consecutive rated quality ladder; NR is unavailable quality, not the notch before D. |
| C.notches_to(D); BBB.notches_to(BB) | +1; +3 library notch counts, not equal increments in default risk. |
| Any notches_to involving NR, including NR/NR | Explicit unavailable-quality error. |
| NR compared with rated value / NR==NR | Incomparable / same status identity, not equal assessed credit risk. |
| BBB- / BB+ / D / NR predicates | IG / speculative / speculative and D marker / neither grade nor D marker. |
| Moody C or Ca normalized | Generic C or CC shorthand; is_default false does not certify a performing obligation. |
| Aaa, Aa1, Baa2, Caa3, Ca | AAA, AA+, BBB, CCC-, CC under the declared symbolic crosswalk. |
| Aa without a modifier | Generic AA via case-insensitive token overlap; no original Aa2 observation is inferred. |
| AAA+, C3, D1, NR-, BBB2, CAA+, bare Baa/Ba/Caa | Error rather than discarded suffixes/invented middle grades. |
| SD, RD, withdrawn, outlook/watch or national/short-term modifiers | Unsupported; explicit owner treatment required, no automatic D/NR. |
| Human alias DEFAULT; wire DEFAULT | Human D marker / canonical-wire label error. |
| Moody rendering of D / NR | Unsupported grade error / explicit unrated status text, not an assigned opinion. |
| Scorecard registry/scale missing, misspelled or unknown | Error when required; no embedded sp/score 50/warn fallback. |
| Explicit custom scale ID not named sp or moodys | Valid if present and fully validated; no two-brand hardcoded restriction. |
| Scale IDs Foo and ' foo '; labels AAA and ' aaa ' | Normalized-identity collision error. |
| Illustrative internal bands High>=80, Medium>=40, Low>=0 | Full-domain bands [80,100],[40,80),[0,40); example only, not a default or agency calibration. |
| Scores80,40,0,100 on those illustrative bands | High,Medium,Low,High; no epsilon or missing-status inference. |
| Medium representative80 with High minimum80 | Reject: its representative classifies into the better band. |
| Lowest minimum0.01, NaN/out-of-range score, duplicate/unsorted levels | Validation/classification error, not last-band/clipped/repaired output. |
| Embedded AA+ representative95 / BB+50 / C2 under old sp minima | Currently classify as AAA / BBB- / CC; unsupported defaults are retired, not certified. |
| Numeric NR/unrated band or unknown threshold/min-rating label | Error; missing status is not a number or false compliance result. |
| Snapshot selected once, then caller changes another registry value | Existing scorecard retains its captured immutable policy; no global/default replacement. |
| Scorecard label D or Moody-style C | Application utility label alone does not establish an agency assignment/default event/PD. |
| Full registry versus to_table | Validated reproducible policy data/provenance versus an ordered presentation view. |
| Invalid JSON/config/host update | Atomic error; no partial mutation or compatibility restoration. |

## Observed algorithm: rating classification and normalization

Generic ratings order from AAA=0 through C=20, **NR=21**, D=22. Investment grade is ordinal <=BBB-=9. Speculative grade is anything else except NR, so D is speculative and default. Default matches D only. `notches_to(other)` subtracts ordinals, so downgrade is positive and movements involving NR affect distance. `bucket` removes +/- within AA/A/BBB/BB/B/CCC only.

Parser trims, removes internal ordinary spaces, uppercases, accepts NR/NOTRATED/UNRATED and D/DEFAULT, then strips +/- or final 1/2/3 as plus/flat/minus. Map BAA to BBB, BA to BB, CAA to CCC, CA to CC. AAA/CC/C/D/NR ignore parsed notch suffixes rather than rejecting them, so nonstandard strings such as AAA+ or C3 can normalize. Moody's rendering maps notched buckets to 1/2/3, but D/NR remain literal generic labels.

## Observed algorithm: scorecard registry

Embedded schema is `finstack_quant.rating_scales/2`, with default score 50, default scale `sp`, and unknown ID policy FallbackToDefault. Normalize IDs and default ID by trim/lowercase, require unique IDs and a present default scale. Rating levels require nonblank names and finite score/min_score in [0,100]. Scales require nonempty ratings, exact-name uniqueness and strictly descending score and min_score; validation does not require each representative score to lie above its own threshold or below the preceding threshold.

Embedded S&P names AAA through CCC- use representative scores 100,95,...,10 and lower thresholds 95,90,...,5; CC uses score 5/min 2, C score 2/min 0.01, D score 0/min 0. Moody's corresponding Aaa through Caa3 use the same descending 5-point sequence; Ca=5/min2 and C=2/min0.01, with no D row. These are application scorecard scales and not the CreditRating ordinal numbers.

`rating_scale(name)` first resolves exact normalized ID. UnknownScalePolicy Error rejects; FallbackToDefault silently returns default; WarnAndFallback logs then returns default. Config extension `core.rating_scales.v1` supplies a complete registry using the same schema/validation. `to_table` preserves ordered name/score/min_score columns and computes no rating from an input score in this slice.

## Observed limitations and alternatives

Public level/scale fields allow postconstruction mutations; deserialization and explicit constructors validate. Same names differing in case/whitespace can coexist because duplicate checking uses exact strings after only a blank-name test. Registry metadata source strings identify agencies but contain no derivation for 0–100 scores. NR is an ordinal state, not a mathematically missing rank. Supported alternatives are custom validated scales and explicit unknown-scale error policy; PD-based mapping or agency-preserving incomparable grades are not implemented.

## Evidence

[Actual scorecard consumer](../../../../finstack-quant/statements-analytics/src/extensions/scorecards/mod.rs)
loads `embedded_registry` directly even though a config-replacement loader exists.
Its best-first inclusive-minimum classifier maps existing representative AA+95 to
AAA, BB+50 to BBB- and C2 to CC. An independent reference calculation over the
43 shipped rows found 40 representatives classify into another band. The Moody
scale's lowest minimum 0.01 also leaves [0,0.01) outside its declared minima,
currently hidden by the consumer's last-row fallback. These are source/data
consistency findings, not executed qualification of a firm's scorecard or a
calibrated agency model. The
registry's score 50 field exists, but the located metric-gap code does not use
that accessor as a gap-imputation rule; do not infer that behavior from its name.

[Rating-factor containers](../../../../finstack-quant/models/src/credit/rating_factors.rs),
[margin grade thresholds](../../../../finstack-quant/margin/src/calculators/im/haircut.rs),
[structured-credit coverage](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/coverage.rs),
[Python generic types](../../../../finstack-quant-py/src/bindings/core/types.rs),
[Python registry](../../../../finstack-quant-py/src/bindings/core/rating_scales.rs),
[WASM generic types](../../../../finstack-quant-wasm/src/api/core/types.rs)
and [WASM registry](../../../../finstack-quant-wasm/src/api/core/rating_scales.rs)
locate comparison/key/state/host changes. Their source-selected NR assumptions
and empirical data are not approved by this core representation package.

[S&P's rating overview](https://www.spglobal.com/ratings/en/credit-ratings/about/understanding-credit-ratings)
identifies the BBB-/BB+ boundary and includes D in speculative grades.
[Moody's long-term definitions](https://www.moodys.com/web/en/us/solutions/ratings/understanding-ratings.html)
show numeric modifiers for Aa through Caa, a lowest long-term C grade, and default-related
interpretations of Ca/C rather than a D grade. [Fitch's 19 September 2025 definitions](https://assets.fitchratings.com/downloadFile?reportType=report&sfReport=false&slug=banks/rating-definitions-19-09-2025)
describe ordinal ratings without a specified default frequency and distinguish
issuer/obligation/default-state scopes. These sources support narrow symbol/grade
background only, not the library crosswalk's quantitative equivalence, internal
0–100 utility data or a complete current agency/regulatory data qualification.

[Generic ratings](../../../../finstack-quant/core/src/types/ratings.rs), [scale registry](../../../../finstack-quant/core/src/rating_scales.rs), embedded data (`core/data/rating_scales/rating_scales.v2.json`, removed on 2026-11-12 by the reconciliation). Inline tests cover grade boundaries, signed/antisymmetric notch differences, stable labels, scale order validation, unknown-scale policies and config loading. These are representation/invariant tests. [Moody's symbols](../../../REFERENCES.md#moodys-rating-symbols) provides naming background only; it does not support the numeric score spacing/default fallback.

Primary symbol/grade sources were consulted on 2026-11-07. Independent reference
checks passed for the 22-state rated ladder, 10 IG/12 speculative states, literal
alias acceptance/rejection, signed notch examples and illustrative full-domain
score bands at boundaries and their adjacent binary64 values. Independent semantic
reviews of the generic-rating and registry/consumer packages found no material
inconsistency. These are documentation/reference checks, not execution of the
accepted Rust API contracts. Quantitative calibration, full agency-status/vendor/regulatory
qualification and production/host tests remain pending.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
