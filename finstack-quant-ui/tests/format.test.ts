import { expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import init, { core } from "../../finstack-quant-wasm/index.js";
import {
  convertRate,
  formatRate,
  rateToWire,
  formatMoney,
  groupDecimal,
  formatRaw,
  formatSigned,
  isoToEpoch,
  epochToIso,
  type RoundingStamp,
} from "../src/format/format";
import {
  moneyColumn,
  rateColumn,
  dateColumn,
  idColumn,
  signedColumn,
} from "../src/format/columns";
import type { ColumnDef } from "@tanstack/react-table";
await init({
  module_or_path: await readFile(
    new URL(
      "../../finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm",
      import.meta.url,
    ),
  ),
});
const presentation = {
  source: "finstack-quant-wasm/index.d.ts#Rate",
  wire: "decimal",
  display: "bp",
} as const;
it("reverses 20-digit fractional presentation without a number conversion", () => {
  const value = "0.12345678901234567890";
  const display = formatRate(value, presentation);
  expect(display).toEqual({ text: "1234.5678901234567890", unit: "bp" });
  expect(rateToWire(display.text, presentation)).toBe("0.12345678901234567890");
  expect(convertRate("12345678901234567890.12345", "decimal", "percent")).toBe(
    "1234567890123456789012.345",
  );
  expect(convertRate("1234567890123456789012.345", "percent", "decimal")).toBe(
    "12345678901234567890.12345",
  );
  expect(value).toBe("0.12345678901234567890");
});
it("formats small native numeric rates without exponent parsing loss", () => {
  expect(formatRate(1e-7, presentation)).toEqual({ text: "0.001", unit: "bp" });
});
it("leaves unsupported-unit values and their spelling unchanged", () => {
  expect(formatRate("0.12345678901234567890")).toEqual({
    text: "0.12345678901234567890",
    unit: null,
  });
  expect(rateToWire("001.2500")).toBe("001.2500");
  expect(convertRate("001.2500", "decimal", "decimal")).toBe("001.2500");
  expect(() => formatRate("0.1", { ...presentation, source: "" })).toThrow(
    /source/,
  );
});
it("groups decimal text exactly and preserves the wire string", () => {
  expect(groupDecimal("12345678901234567890.1234500")).toBe(
    "12,345,678,901,234,567,890.1234500",
  );
  expect(groupDecimal("-0.00100")).toBe("-0.00100");
  expect(() => groupDecimal("1,000.00")).toThrow(/decimal/);
  const wire = { amount: "12345678901234567890.12345", currency: "USD" };
  expect(formatMoney(wire)).toBe("USD 12,345,678,901,234,567,890.12345");
  expect(wire.amount).toBe("12345678901234567890.12345");
  const native = core.Money.fromJson(JSON.stringify(wire));
  try {
    expect(native.amountDecimal()).toBe(wire.amount);
  } finally {
    native.free();
  }
});
it.each([
  ["bankers", "1.245", "USD 1.24"],
  ["bankers", "1.255", "USD 1.26"],
  ["away_from_zero", "-1.245", "USD -1.25"],
  ["away_from_zero", "1.241", "USD 1.24"],
  ["toward_zero", "-1.259", "USD -1.25"],
  ["floor", "-1.241", "USD -1.25"],
  ["floor", "1.249", "USD 1.24"],
  ["ceil", "1.241", "USD 1.25"],
  ["ceil", "-1.249", "USD -1.24"],
] as const)("honours returned %s rounding for %s", (mode, amount, expected) => {
  const stamp: RoundingStamp = { mode, output_scale_by_currency: { USD: 2 } };
  const money = { amount, currency: "USD" };
  expect(formatMoney(money, stamp)).toBe(expected);
  expect(money.amount).toBe(amount);
});
it("uses currency-specific stamps and preserves digits where no scale was returned", () => {
  const stamp: RoundingStamp = {
    mode: "bankers",
    output_scale_by_currency: { USD: 4, JPY: 0 },
  };
  expect(formatMoney({ amount: "1.23456", currency: "USD" }, stamp)).toBe(
    "USD 1.2346",
  );
  expect(formatMoney({ amount: "1.5", currency: "JPY" }, stamp)).toBe("JPY 2");
  expect(formatMoney({ amount: "1.23456", currency: "KWD" }, stamp)).toBe(
    "KWD 1.23456",
  );
});
it.each(["1970-01-01", "1969-12-31", "2024-02-29", "0001-01-01", "9999-12-31"])(
  "uses native calendar conversion for %s",
  (iso) => {
    expect(epochToIso(isoToEpoch(iso, core), core)).toBe(iso);
  },
);
it("rejects invalid dates and fractional/coerced epoch days", () => {
  expect(isoToEpoch("1970-01-01", core)).toBe(0);
  expect(() => isoToEpoch("2023-02-29", core)).toThrow();
  expect(() => isoToEpoch("2024-13-01", core)).toThrow();
  expect(() => epochToIso(1.5, core)).toThrow(/integer/);
  expect(() => epochToIso(4294967296, core)).toThrow(/integer/);
});
it("keeps scalar display text distinct from canonical JSON", () => {
  expect(formatRaw((1n << 64n) - 1n)).toBe("18446744073709551615");
  expect(formatRaw(null)).toBe("—");
  expect(formatSigned("0.000")).toBe("0.000");
  expect(formatSigned("12345678901234567890.1")).toBe(
    "+12345678901234567890.1",
  );
});
it("provides native Table v9 definitions with raw accessors and presentation metadata", () => {
  type Row = {
    id: string;
    date: string;
    money: { amount: string; currency: string };
    rate: string;
    delta: bigint;
  };
  const row: Row = {
    id: "BOND_A",
    date: "2024-02-29",
    money: { amount: "1.23", currency: "USD" },
    rate: "0.025",
    delta: 2n,
  };
  const columns = [
    moneyColumn("money", (r: Row) => r.money),
    rateColumn("rate", (r: Row) => r.rate, presentation),
    dateColumn("date", (r: Row) => r.date),
    idColumn("id", (r: Row) => r.id),
    signedColumn("delta", (r: Row) => r.delta),
  ];
  const nativeColumns: ColumnDef<{}, Row, any>[] = columns;
  expect(nativeColumns).toHaveLength(5);
  for (const column of columns) {
    expect(column.meta.className).toBeTruthy();
    expect("accessorFn" in column).toBe(true);
  }
  expect("accessorFn" in columns[0] && columns[0].accessorFn(row, 0)).toBe(
    row.money,
  );
  expect(
    columns.every(
      (column) => !("sortingFn" in column) && !("aggregationFn" in column),
    ),
  ).toBe(true);
});
