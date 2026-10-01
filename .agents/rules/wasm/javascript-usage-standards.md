---
trigger: glob
description:
globs: "*.tsx,*.ts,*.js"
---

# JavaScript/TypeScript Usage Standards for finstack-quant-wasm

## Overview

Standards for JavaScript and TypeScript code that uses the finstack-quant-wasm module.

Every `javascript` / `typescript` block in this file is type-checked against
`index.d.ts` by `finstack-quant-wasm/scripts/check-typescript-examples.mjs`
(`mise run wasm-doc`) and executed against the built package by
`finstack-quant-wasm/tests/facade/doc_examples.test.mjs` (`mise run wasm-test`).
A block with no `import` continues from the standard setup — all namespaces
imported and `await init()` done.

## Setup and Initialization

### Browser Setup

```javascript
import init, { core } from "finstack-quant-wasm";

async function initialize() {
  await init();

  const usd = new core.Currency("USD");
  const amount = new core.Money(100.0, usd);
  const date = core.createDate(2024, 1, 15);
  return { amount, date };
}

initialize().catch(console.error);
```

### Node Setup

`index.js` re-exports the **web** target, whose `init()` fetches the `.wasm` by
URL. Node cannot fetch a file URL, so read the bytes and pass them in (see
"Initialization: web vs Node" in `finstack-quant-wasm/README.md`):

```javascript
import { readFileSync } from "node:fs";
import init, { core } from "finstack-quant-wasm";

await init({
  module_or_path: readFileSync(
    new URL(import.meta.resolve("finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm")),
  ),
});
new core.Currency("USD").numeric; // 840
```

### TypeScript Setup

```typescript
import init, { core } from "finstack-quant-wasm";
import type { Money } from "finstack-quant-wasm";

async function example(): Promise<Money> {
  await init();
  const usd = new core.Currency("USD");
  return new core.Money(100.0, usd);
}

await example();
```

## Import Patterns

### Namespaced Imports (Required)

The public API is accessed through crate-domain namespaces, not flat imports.
The default export is the initializer; the named exports are the namespaces:

```javascript
import init, {
  core,
  analytics,
  attribution,
  calibration,
  cashflows,
  covenants,
  features,
  margin,
  models,
  valuations,
  statements,
  statements_analytics,
  portfolio,
  scenarios,
} from "finstack-quant-wasm";

await init();
```

### Usage via Namespaces

```javascript
// Core types
const usd = new core.Currency("USD");
const money = new core.Money(1000.5, usd);
const date = core.createDate(2024, 9, 30);

// Analytics
const perf = analytics.Performance.fromReturns(
  ["2024-01-01", "2024-01-02", "2024-01-03"],
  [[0.01, 0.02, -0.01]],
  ["asset"],
  null,
  "daily",
);
const s = perf.sharpe(0.0); // Float64Array, one value per ticker

// Valuations: typed instruments are built with their Rust constructors
// (`Bond.fixed`, `Bond.floating`, `Bond.zeroCoupon`, `Bond.fromJson`) or the
// fluent `Bond.builder()`. Handle-typed arguments take handles: a notional is
// a `core.Money`, never a bare number.
const bond = valuations.instruments.Bond.fixed(
  "BOND-1",
  new core.Money(1_000_000, usd),
  new core.Rate(0.05),
  "2024-01-01",
  "2034-01-01",
  "none",
  "USD-OIS",
);

// Models (closed-form kernels; Monte Carlo lives under models.monteCarlo)
const price = models.bsPrice(100, 100, 0.03, 0, 0.2, 1, true);
```

### Do NOT import flat from pkg/

`pkg/finstack_quant_wasm.js` is internal wasm-bindgen output: its flat names
are not the public API and change without notice.

```javascript no-run
// WRONG: importing from internal raw output
import { Currency, Money } from "./pkg/finstack_quant_wasm.js";
```

```javascript
// CORRECT: import from the facade
import init, { core } from "finstack-quant-wasm";
await init();
const usd = new core.Currency("USD");
```

## Type Construction

### Currency and Money

```javascript
const usd = new core.Currency("USD");
const eur = new core.Currency("EUR");

console.log(usd.code); // "USD"
console.log(usd.numeric); // 840
console.log(eur.decimals); // 2

const amount = new core.Money(1000.5, usd);
console.log(amount.amount); // 1000.5
```

### Dates

```javascript
const date = core.createDate(2024, 9, 30);

console.log(date); // 19996  (epoch days, NOT an ISO string)

const nextBD = core.adjust(date, "modified_following", "nyse");

// Decompose back into calendar parts when you need them:
const [year, month, day] = core.dateFromEpochDays(nextBD);
```

**The date representation is not uniform across namespaces — check the function
you are calling.** There are two conventions in use:

| Surface | Representation | Example |
| --- | --- | --- |
| `core` date utilities (`createDate`, `adjust`, `dateFromEpochDays`) | **integer epoch days** (days since 1970-01-01) | `core.createDate(2024, 9, 30)` → `19996` |
| Panel/series ingestion (e.g. `analytics.Performance.fromReturns`) | **ISO date strings** | `["2024-01-01", "2024-01-02"]` |

`core.createDate` and `core.adjust` both return `number`, and `core.adjust` expects
that same integer as input. Never pass an ISO string to a `core` date function, and
never pass epoch days to a panel constructor — neither coerces: a wrong type throws a
`TypeError` with `kind: "invalid_type"` whose message starts with the argument name
(`"startEpochDays: expected a number, got string"`).

Use `core.dateFromEpochDays(days)` to convert epoch days back to a
`[year, month, day]` triple. To interoperate with the host `Date` type, convert
explicitly — for example `new Date(epochDays * 86_400_000)` for a UTC instant.

## Error Handling

Bindings throw real `Error` objects. A Rust error arrives named `FinstackError`
with `kind` set from the Rust error type (`validation`, `not_found`,
`computation`, ...); a wrong argument type is a `TypeError` with
`kind: "invalid_type"`. Branch on `kind`, never on the message text.

```javascript
/** @typedef {import("finstack-quant-wasm").FinstackError} FinstackError */

try {
  new core.Currency("XXX");
} catch (caught) {
  const error = /** @type {FinstackError} */ (caught);
  console.error(error.name, error.kind); // "FinstackError", "validation"
}

const money1 = new core.Money(1, new core.Currency("USD"));
const money2 = new core.Money(1, new core.Currency("EUR"));
try {
  money1.checkedAdd(money2);
} catch (caught) {
  const error = /** @type {FinstackError} */ (caught);
  console.error("Operation failed:", error.message); // currency mismatch
}
```

## Testing

### Node Test Runner

Facade tests live in `finstack-quant-wasm/tests/facade/*.test.mjs` and run with
`mise run wasm-test` (build + run) or `mise run wasm-test-built` (run only).
They import `../../index.js` and read the bytes from `../../pkg/`; a consumer
test resolves the same files through the package name:

```javascript
import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import init, { core, analytics } from "finstack-quant-wasm";

await init({
  module_or_path: readFileSync(
    new URL(import.meta.resolve("finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm")),
  ),
});

test("core.Currency creation", () => {
  const usd = new core.Currency("USD");
  assert.equal(usd.code, "USD");
});

test("analytics.Performance.sharpe returns a typed array", () => {
  const perf = analytics.Performance.fromReturns(
    ["2024-01-01", "2024-01-02"],
    [[0.01, 0.02]],
    ["asset"],
    null,
    "daily",
  );
  const value = perf.sharpe(0.0);
  assert.equal(value instanceof Float64Array, true);
});
```

## Performance

- Reuse objects (Currency, DayCount) rather than recreating.
- Batch operations to minimize JS↔WASM boundary crossings.
- Avoid creating temporary objects in tight loops.
- Every wasm-bindgen class instance owns WASM memory: call `free()` (or use
  `using` where `Symbol.dispose` is available) on handles created in a loop.

## Documentation

The namespaces (`core`, `analytics`, ...) are values. The handle types are
top-level type exports of the package, so JSDoc names them through `import()`:

```javascript
/**
 * @param {import("finstack-quant-wasm").Currency} currency
 * @param {number} amount
 * @returns {import("finstack-quant-wasm").Money}
 */
function createMoney(currency, amount) {
  return new core.Money(amount, currency);
}

createMoney(new core.Currency("USD"), 100).amount; // 100
```
