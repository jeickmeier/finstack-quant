---
trigger: glob
description:
globs: *.tsx,*.ts,*.js
---
# JavaScript/TypeScript Usage Standards for finstack-wasm

## Overview

Standards for JavaScript and TypeScript code that uses the finstack-wasm module.

## Setup and Initialization

### Browser Setup

```javascript
import init, { core, analytics, valuations, margin } from "finstack-wasm";

async function initialize() {
    await init();

    const usd = new core.Currency("USD");
    const amount = new core.Money(100.0, usd);
    const date = new core.dates.FsDate(2024, 1, 15);
}

initialize().catch(console.error);
```

### TypeScript Setup

```typescript
import init, {
  core, analytics, correlation, margin, monte_carlo,
  portfolio, scenarios, statements, statements_analytics, valuations
} from "finstack-wasm";

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
  margin,
  valuations,
  statements,
  statements_analytics,
  portfolio,
  scenarios,
  correlation,
  monte_carlo,
} from "finstack-wasm";
```

### Usage via Namespaces

```javascript
await init();

// Core types
const usd = new core.Currency("USD");
const money = new core.Money(1000.50, usd);
const date = new core.dates.FsDate(2024, 9, 30);

// Analytics
const s = analytics.sharpe([0.01, 0.02, -0.01], 0.0);

// Valuations
const bond = valuations.instruments.Bond.builder()
    .notional(1000000)
    .build();

// Monte Carlo
const grid = new monte_carlo.TimeGrid([0.0, 0.5, 1.0]);
```

### Do NOT import flat from pkg/

```javascript
// WRONG: importing from internal raw output
import { Currency, Money } from "./pkg/finstack_wasm.js";

// CORRECT: import from the facade
import init, { core } from "finstack-wasm";
const usd = new core.Currency("USD");
```

## Type Construction

### Currency and Money

```javascript
const usd = new core.Currency("USD");
const eur = new core.Currency("EUR");

console.log(usd.code);         // "USD"
console.log(usd.numericCode);  // 840

const amount = new core.Money(1000.50, usd);
console.log(amount.amount);    // 1000.5
```

### Dates

```javascript
const date = new core.dates.FsDate(2024, 9, 30);

console.log(date.year);    // 2024
console.log(date.month);   // 9
console.log(date.day);     // 30

const isWeekend = date.isWeekend();
const nextBD = date.addBusinessDays(1);
```

Note: WASM uses `FsDate` (not `Date`) to avoid collision with JavaScript's built-in `Date`.

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
import init, { core, analytics } from "finstack-wasm";

await init();

test("core.Currency creation", () => {
    const usd = new core.Currency("USD");
    assert.equal(usd.code, "USD");
});

test("analytics.sharpe returns float", () => {
    const value = analytics.sharpe([0.01, 0.02], 0.0);
    assert.equal(typeof value, "number");
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
