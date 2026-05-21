# Types module

`finstack_core::types` holds small, reusable scalar and domain types:

- Phantom-typed identifiers (`id.rs`)
- Rate and unit wrappers (`rates.rs`)
- Credit-rating helpers (`ratings.rs`)
- Lightweight attribute bags (`attributes.rs`)

This module does **not** re-export `Currency`, `Date`, `OffsetDateTime`,
`PrimitiveDateTime`, or `HashMap`. Import those from their owning modules.

## Public exports

From `types/mod.rs`:

- **IDs**: `Id`, `CalendarId`, `CurveId`, `DealId`, `IndexId`, `InstrumentId`,
  `IssuerId`, `PoolId`, `PriceId`, `UnderlyingId`
- **Rates**: `Rate`, `Bps`, `Percentage`
- **Ratings**: `CreditRating`, `RatingLabel`, `RatingFactorTable`, `moodys_warf_factor`
- **Metadata**: `Attributes`

## Usage

### Typed identifiers

```rust
use finstack_core::types::{CurveId, InstrumentId};

let curve_id = CurveId::from("USD-OIS");
let instrument_id = InstrumentId::from("US912828XG60");

assert_eq!(curve_id.as_str(), "USD-OIS");
assert_eq!(instrument_id.as_str(), "US912828XG60");
```

### Rates and percentages

```rust
use finstack_core::types::{Bps, Percentage, Rate};

let rate = Rate::from_percent(5.0);
let spread = Bps::new(25);
let pct = Percentage::new(12.5);

assert_eq!(rate.as_decimal(), 0.05);
assert_eq!(spread.as_decimal(), 0.0025);
assert_eq!(pct.as_decimal(), 0.125);
```

### Credit ratings

```rust
use finstack_core::types::{CreditRating, RatingLabel, moodys_warf_factor};

let rating: CreditRating = "Baa3".parse().expect("valid rating");
assert_eq!(rating, CreditRating::BBBMinus);

let label = RatingLabel::moodys(CreditRating::BBBMinus);
assert_eq!(label.as_str(), "Baa3");

let factor = moodys_warf_factor(CreditRating::B).unwrap();
assert_eq!(factor, 2720.0);
```

## Conventions

- `Rate` stores decimal rates (`5%` → `0.05`)
- `Bps` stores basis points (`25 bp` → `25.0`)
- `Percentage` stores whole-percent values (`25%` → `25.0`)
- Typed IDs use `Arc<str>` internally; serialization uses the string form

New names added to `types/mod.rs` are long-lived public API.
