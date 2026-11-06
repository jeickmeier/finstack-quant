# MODELS-NUMERICAL-002: Escrowed cash dividends and lattice exercise ordering

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the owning paths identified below; no implementation change authorized.
- Validation status: Evidence inspected/located, **not executed**. Formula comparisons, invariants and roundtrips are distinguished below; none establishes universal accuracy.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact scope ledger in [coverage.json](coverage.json).

## Choice and input domain

The [binomial option valuator](../../../../finstack-quant/models/src/trees/binomial_tree.rs) evolves an escrowed component to retain recombination with known cash dividends. Dividends are positive finite cash amounts in spot units at times in (0,T]; continuous yield remains active. Empty schedules delegate to the ordinary stock tree. This is a model approximation for the stochastic stock component, not merely a cashflow bookkeeping identity.

## Algorithm, controls and conventions

Map each dividend time to round(N·t/T), clamped to [1,N]. Reserve PV is ΣD exp(−(r−q)t) using original times, and X0=S0−PV must be positive finite. Construct CRR or LR using X0 (LR remains strike centered), then evolve X with the same r−q carry and input volatility. At a node add reserves whose mapped dividend steps are later for ex-dividend spot, or later/equal for cum-dividend spot. Reserve discount time uses max(original time−node time,0).

At maturity pay intrinsic on the ex-dividend spot. At eligible interior nodes return max(discounted continuation, intrinsic(ex spot), intrinsic(cum spot)). American selects steps 0..N−1. Bermudan delegates date mapping to core `map_exercise_dates_to_steps`, sorts and deduplicates the result. The engine does not insert exact dates or substeps. `price_european` has no cash-dividend argument.

## Limitations and alternatives

Exercise immediately before and after mapped cash events is represented by a maximum on one node; actual cash times still enter the reserve. The approximation combines original dividend times in reserves with mapped times in exercise and applies volatility to the escrow component. Invalid dividend times/amounts and a reserve exhausting spot return errors. There is no spot floor fallback. An event-aligned non-recombining tree or jump-interpolated PDE would be alternative implementations, not enabled variants. Date validation inside the external core mapper belongs to that owning slice; callers should not infer the exact-date contract of the Hull–White tree.

## Evidence

Inline `cash_dividend_tree_preserves_continuous_carry_in_deterministic_limit` checks the zero-volatility carry identity; `vanishing_cash_dividend_converges_to_continuous_yield_price` checks the small-cash limit; `cash_dividend_at_maturity_is_ex_dividend_at_expiry` checks expiry ordering. `test_bermudan_between_european_and_american` is an inequality check, not an independent American benchmark. [Hull textbook](../../../REFERENCES.md#hull-options-futures) is general background; it does not independently validate this specific escrow/exercise convention.

## Follow-up tracking

Adoption questions, implementation reconciliation and validation work are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#trees-and-finite-differences).
