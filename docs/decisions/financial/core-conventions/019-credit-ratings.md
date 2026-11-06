# CORE-CONVENTIONS-019: Ordinal credit ratings and configurable scorecard scale data

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `types/ratings.rs`, `rating_scales.rs` and embedded `rating_scales.v2.json`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

Generic ordinal ratings and scorecard numeric scales are separate representations. Neither implementation derives default probabilities, transition intensities or agency-calibrated risk weights. The embedded scores are application data; agency naming does not establish quantitative agency provenance.

## Algorithm: rating classification and normalization

Generic ratings order from AAA=0 through C=20, **NR=21**, D=22. Investment grade is ordinal <=BBB-=9. Speculative grade is anything else except NR, so D is speculative and default. Default matches D only. `notches_to(other)` subtracts ordinals, so downgrade is positive and movements involving NR affect distance. `bucket` removes +/- within AA/A/BBB/BB/B/CCC only.

Parser trims, removes internal ordinary spaces, uppercases, accepts NR/NOTRATED/UNRATED and D/DEFAULT, then strips +/- or final 1/2/3 as plus/flat/minus. Map BAA to BBB, BA to BB, CAA to CCC, CA to CC. AAA/CC/C/D/NR ignore parsed notch suffixes rather than rejecting them, so nonstandard strings such as AAA+ or C3 can normalize. Moody's rendering maps notched buckets to 1/2/3, but D/NR remain literal generic labels.

## Algorithm: scorecard registry

Embedded schema is `finstack_quant.rating_scales/2`, with default score 50, default scale `sp`, and unknown ID policy FallbackToDefault. Normalize IDs and default ID by trim/lowercase, require unique IDs and a present default scale. Rating levels require nonblank names and finite score/min_score in [0,100]. Scales require nonempty ratings, exact-name uniqueness and strictly descending score and min_score; validation does not require each representative score to lie above its own threshold or below the preceding threshold.

Embedded S&P names AAA through CCC- use representative scores 100,95,...,10 and lower thresholds 95,90,...,5; CC uses score5/min2, C score2/min0.01, D score0/min0. Moody's corresponding Aaa through Caa3 use the same descending 5-point sequence; Ca=5/min2 and C=2/min0.01, with no D row. These are application scorecard scales and not the CreditRating ordinal numbers.

`rating_scale(name)` first resolves exact normalized ID. UnknownScalePolicy Error rejects; FallbackToDefault silently returns default; WarnAndFallback logs then returns default. Config extension `core.rating_scales.v1` supplies a complete registry using the same schema/validation. `to_table` preserves ordered name/score/min_score columns and computes no rating from an input score in this slice.

## Limitations and alternatives

Public level/scale fields allow postconstruction mutations; deserialization and explicit constructors validate. Same names differing in case/whitespace can coexist because duplicate checking uses exact strings after only a blank-name test. Registry metadata source strings identify agencies but contain no derivation for 0–100 scores. NR is an ordinal state, not a mathematically missing rank. Supported alternatives are custom validated scales and explicit unknown-scale error policy; PD-based mapping or agency-preserving incomparable grades are not implemented.

## Evidence

[Generic ratings](../../../../finstack-quant/core/src/types/ratings.rs), [scale registry](../../../../finstack-quant/core/src/rating_scales.rs), [embedded data](../../../../finstack-quant/core/data/rating_scales/rating_scales.v2.json). Inline tests cover grade boundaries, signed/antisymmetric notch differences, stable labels, scale order validation, unknown-scale policies and config loading. These are representation/invariant tests. [Moody's symbols](../../../REFERENCES.md#moodys-rating-symbols) provides naming background only; it does not support the numeric score spacing/default fallback.

External agency-rule freshness was not verified.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
