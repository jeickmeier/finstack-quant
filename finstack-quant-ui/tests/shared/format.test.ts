import { expect, it } from "vitest";
import { readFile } from "node:fs/promises";
import init, { core } from "../../../finstack-quant-wasm/index.js";
import type { MoneyValue } from "finstack-quant-wasm";
import {
  formatPercent,
  formatMoney,
  formatRawMoney,
  groupDecimal,
  formatRaw,
  formatSigned,
  isoToEpoch,
  epochToIso,
  compareCoordinates,
  type RoundingStamp,
} from "../../src/format/format";
import {
  moneyColumn,
  dateColumn,
  idColumn,
  signedColumn,
} from "../../src/format/columns";
import type { ColumnDef } from "@tanstack/react-table";
await init({
  module_or_path: await readFile(
    new URL(
      "../../../finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm",
      import.meta.url,
    ),
  ),
});
it("displays supplied fractions as exact percent digits", () => {
  expect(formatPercent(0.25)).toBe("25");
  expect(formatPercent(0.125)).toBe("12.5");
  expect(formatPercent(1e-7)).toBe("0.00001");
  expect(formatPercent(-0.05)).toBe("-5");
  expect(formatPercent(0.07)).toBe("7");
});
it("groups decimal text exactly and preserves the wire string", () => {
  expect(groupDecimal("12345678901234567890.1234500")).toBe(
    "12,345,678,901,234,567,890.1234500",
  );
  expect(groupDecimal("-0.00100")).toBe("-0.00100");
  expect(() => groupDecimal("1,000.00")).toThrow(/decimal/);
  const wire = {
    amount: "12345678901234567890.12345",
    currency: "USD" as const,
  };
  expect(formatRawMoney(wire)).toBe("USD 12,345,678,901,234,567,890.12345");
  expect(wire.amount).toBe("12345678901234567890.12345");
  const native = core.Money.fromJson(JSON.stringify(wire));
  try {
    expect(native.amountDecimal).toBe(wire.amount);
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
  const money = { amount, currency: "USD" as const };
  expect(formatMoney(money, stamp, core)).toBe(expected);
  expect(money.amount).toBe(amount);
});
it("uses currency-specific stamps and preserves digits where no scale was returned", () => {
  const stamp: RoundingStamp = {
    mode: "bankers",
    output_scale_by_currency: { USD: 4, JPY: 0 },
  };
  expect(
    formatMoney({ amount: "1.23456", currency: "USD" as const }, stamp, core),
  ).toBe("USD 1.2346");
  expect(formatMoney({ amount: "1.5", currency: "JPY" }, stamp, core)).toBe(
    "JPY 2",
  );
  expect(formatMoney({ amount: "1.23456", currency: "KWD" }, stamp, core)).toBe(
    "KWD 1.23456",
  );
});
it("rejects inexact amount text through the native constructor", () => {
  const stamp: RoundingStamp = {
    mode: "bankers",
    output_scale_by_currency: { USD: 2 },
  };
  expect(() =>
    formatMoney(
      { amount: "1.24500000000000000000000000001", currency: "USD" as const },
      stamp,
      core,
    ),
  ).toThrow(/exactly representable/);
});
it.each([
  { mode: undefined, output_scale_by_currency: { USD: 2 } },
  { mode: null, output_scale_by_currency: { USD: 2 } },
  { mode: "ceil", output_scale_by_currency: { USD: null } },
  { mode: "ceil", output_scale_by_currency: { USD: "2" } },
])(
  "rejects malformed returned stamp %j before native construction",
  (stamp) => {
    expect(() =>
      formatMoney(
        { amount: "1.245", currency: "USD" as const },
        stamp as unknown as RoundingStamp,
        core,
      ),
    ).toThrow(new TypeError("Invalid returned rounding stamp"));
  },
);
it("keeps arbitrary-precision text raw without a scale stamp", () => {
  const wide = {
    amount: "1.24500000000000000000000000001",
    currency: "USD" as const,
  };
  expect(formatRawMoney(wide)).toBe("USD 1.24500000000000000000000000001");
  expect(
    formatMoney(wide, { mode: "bankers", output_scale_by_currency: {} }, core),
  ).toBe("USD 1.24500000000000000000000000001");
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
    money: MoneyValue;
    rate: string;
    delta: bigint;
  };
  const row: Row = {
    id: "BOND_A",
    date: "2024-02-29",
    money: { amount: "1.23", currency: "USD" as const },
    rate: "0.025",
    delta: 2n,
  };
  const columns = [
    moneyColumn("money", (r: Row) => r.money),
    dateColumn("date", (r: Row) => r.date),
    idColumn("id", (r: Row) => r.id),
    signedColumn("delta", (r: Row) => r.delta),
  ];
  const nativeColumns: ColumnDef<{}, Row, any>[] = columns;
  expect(nativeColumns).toHaveLength(4);
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

it("expands scientific decimal text when grouping", () => {
  expect(groupDecimal("1.2345e3")).toBe("1,234.5");
});

it("orders coordinates by Rust's group, then value, then label", () => {
  const at = (label: string, group: string, value: number | null) => ({
    label,
    group,
    value,
  });
  const sorted = (coordinates: ReturnType<typeof at>[]) =>
    [...coordinates].sort(compareCoordinates).map(({ label }) => label);
  // Values as MetricCoordinate supplies them; months are N/12.
  expect(
    sorted([
      at("10y", "", 10),
      at("3m", "", 0.25),
      at("1y", "", 1),
      at("6m", "", 0.5),
    ]),
  ).toEqual(["3m", "6m", "1y", "10y"]);
  // `12m` and `1y` tie on value, so the label decides whatever the input order.
  expect(sorted([at("12m", "", 1), at("1y", "", 1)])).toEqual(["1y", "12m"]);
  expect(sorted([at("1y", "", 1), at("12m", "", 1)])).toEqual(["1y", "12m"]);
  expect(sorted([at("USD-OIS", "USD-OIS", null), at("2y", "", 2)])).toEqual([
    "2y",
    "USD-OIS",
  ]);
  // Decimal strikes order by value, where numeric text collation would tie them.
  expect(
    sorted([
      at("0.04", "", 0.04),
      at("0.08", "", 0.08),
      at("0.004", "", 0.004),
    ]),
  ).toEqual(["0.004", "0.04", "0.08"]);
  // Vega rows group by surface, and case never outranks a later value.
  expect(
    sorted([
      at("S::1y", "S::", 1),
      at("S::6m", "S::", 0.5),
      at("R::3m", "R::", 0.25),
    ]),
  ).toEqual(["R::3m", "S::6m", "S::1y"]);
  expect(
    sorted([at("Usd::2y", "Usd::", 2), at("USD::1y", "USD::", 1)]),
  ).toEqual(["USD::1y", "Usd::2y"]);
  // Labels with no number collate as text.
  expect(
    sorted([
      at("USD-SOFR", "USD-SOFR", null),
      at("EUR-ESTR", "EUR-ESTR", null),
      at("USD-OIS", "USD-OIS", null),
    ]),
  ).toEqual(["EUR-ESTR", "USD-OIS", "USD-SOFR"]);
});
