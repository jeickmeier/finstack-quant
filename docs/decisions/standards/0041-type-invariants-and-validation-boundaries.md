# ADR 0041: Construction and mutation preserve declared type invariants

**Status:** Accepted

**Decision date:** 2026-11-05

**Category:** Code standards

## Decision

A type advertised as valid establishes and preserves its invariants through
every supported construction and mutation path. Raw input and configuration
types have an explicit validation boundary before computation.

Constructors and builders establish the invariants promised by the resulting
type. Deserialization and alternate constructors preserve those same
guarantees for validated types.

Input records may contain incomplete or invalid data when their status and
required validation boundary are clear. Conditions requiring market data or
relationships between objects are checked where that context becomes
available. A successful local construction does not certify conditions that
require later context.

Updates preserve declared invariants; a failed update leaves the object in a
valid state. Any stronger rollback guarantee belongs to the operation's
documented contract.

Restrict field access where necessary to protect invariants. Use additional
wrapper types only when they serve a concrete purpose. Document what
construction guarantees, what later operations must validate, and test each
supported construction, loading, and mutation path.

This establishes a consistent rule for describing and protecting validity
guarantees. It does not claim that all existing types already follow one
construction pattern or require every raw input record to become a separate
validated wrapper.

## Rationale

Callers must know whether an object can be used immediately or still requires
validation. A constructor's checks provide little protection if loading or
mutation bypasses the same promised invariant. Separating local validity from
context-dependent conditions makes the guarantee precise without requiring
unavailable market context during ordinary construction.

## Examples

| Type or path | Treatment |
| --- | --- |
| A rate type promises a finite value | Direct construction and deserialization both reject non-finite inputs. |
| A builder returns a validated curve | Establish the curve's declared local invariants before returning it. |
| A raw request holds incomplete calibration inputs | Allowed when its raw status and validation boundary before computation are explicit. |
| Validating an instrument requires a referenced curve to exist in a market snapshot | Check that relationship when the snapshot is available. |
| A public field permits bypassing a validated type's invariant | Restrict access or provide mutation that preserves the guarantee. |
| A failed update leaves a reusable object outside its declared valid state | Violates this decision. |
| Add wrapper layers without a concrete validation or usage purpose | Not required by this standard; preserve implementation simplicity. |

## Enforcement

- Identify each type's declared local invariants and any remaining contextual
  checks. Make raw input status and validation entry points explicit.
- Test supported constructors, builder completion, alternate input paths, and
  deserialization against the same promised invariants.
- Test successful and failed updates for preservation of the object's declared
  valid state and any separately promised rollback behavior.
- Check that computation paths validate raw inputs and contextual conditions
  before relying on them, including paths reached through host bindings.
- Keep domain validation in Rust under
  [ADR 0001](../architecture/0001-rust-owned-financial-behavior.md), preserving
  host parity under [ADR 0005](0005-python-wasm-parity.md).
- Apply serialized-contract requirements under
  [ADR 0012](0012-serialized-contracts.md), recoverable errors under
  [ADR 0036](0036-safe-rust-and-recoverable-failures.md), and substantive
  documentation under [ADR 0038](0038-substantive-api-documentation.md).
- Preserve scoped authorization for changes to established public contracts
  under [ADR 0015](../project/0015-authorized-change-scope.md).

## Implementation notes and references

- [Rate](../../../finstack-quant/core/src/types/rates.rs) uses private storage
  and routes deserialization through its finite-value constructor.
- [DiscountCurve](../../../finstack-quant/core/src/market_data/term_structures/discount_curve/mod.rs)
  uses a raw representation and checked conversion for deserialization.
- [INVARIANTS.md](../../../INVARIANTS.md), section 8, and
  [ADR 0047](../architecture/0047-credit-factor-wire-and-validation-contract.md)
  explicitly state that Rust deserialization of `CreditFactorModel` does not
  itself call `validate()`; consumers or validating loaders must do so before
  trusting the model. That boundary must remain explicit and effective when
  assessing this type.

These are examples of existing validation patterns, not a complete audit.
Type classification, boundary coverage, and any required reconciliation are
tracked in
[FOLLOW_UPS.md](../FOLLOW_UPS.md#implementation-and-lifecycle-follow-up).
