---
trigger: glob
description:
globs: "*.tsx,*.ts,*.js"
---

# JavaScript/TypeScript Usage Standards for finstack-quant-wasm

## Overview

Standards for JavaScript and TypeScript code that uses the finstack-quant-wasm module.

## Setup and Initialization

### Browser Setup

```javascript
import init, { core, analytics, valuations, margin } from "finstack-quant-wasm";

async function initialize() {
  await init();

  const usd = new core.Currency("USD");
  const amount = new core.Money(100.0, usd);
  const date = core.createDate(2024, 1, 15);
}

initialize().catch(console.error);
```

### TypeScript Setup

```typescript
import init, {
  core,
  analytics,
  calibration,
  margin,
  models,
  portfolio,
  scenarios,
  statements,
  statements_analytics,
  valuations,
} from "finstack-quant-wasm";

async function example(): Promise<void> {
  await init();
  const usd = new core.Currency("USD");
  const money = new core.Money(100.0, usd);
}
```

## Import Patterns

### Namespaced Imports (Required)

The public API is accessed through crate-domain namespaces, not flat imports:

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
```

### Usage via Namespaces

```javascript
await init();

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
const s = perf.sharpe(0.0);

// Valuations
const bond = valuations.instruments.Bond.builder().notional(1000000).build();

// Models (closed-form kernels; Monte Carlo lives under models.monteCarlo)
const price = models.bsPrice(100, 100, 0.03, 0, 0.2, 1, true);
```

### Do NOT import flat from pkg/

```javascript
// WRONG: importing from internal raw output
import { Currency, Money } from "./pkg/finstack_quant_wasm.js";

// CORRECT: import from the facade
import init, { core } from "finstack-quant-wasm";
const usd = new core.Currency("USD");
```

## Type Construction

### Currency and Money

```javascript
const usd = new core.Currency("USD");
const eur = new core.Currency("EUR");

console.log(usd.code); // "USD"
console.log(usd.numericCode); // 840

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

```javascript
try {
  const invalid = new core.Currency("XXX");
} catch (error) {
  console.error("Invalid currency:", error);
}

try {
  const result = money1.add(money2);
} catch (error) {
  console.error("Operation failed:", error);
}
```

## Testing

### Node Test Runner

```javascript
import test from "node:test";
import assert from "node:assert/strict";
import init, { core, analytics } from "finstack-quant-wasm";

await init();

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

## Documentation

Use JSDoc with namespace paths:

```javascript
/**
 * @param {core.Currency} currency
 * @param {number} amount
 * @returns {core.Money}
 */
function createMoney(currency, amount) {
  return new core.Money(amount, currency);
}
```
