// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import {
  MarketContextBrowser,
  marketTree,
  flattenMarket,
  marketPointer,
} from "../registry/components/market-context-browser/market-context-browser";
import { FxMatrixGrid } from "../registry/components/fx-matrix-grid/fx-matrix-grid";
import type { MarketContextStateWire } from "../src/generated/types/market_context_state";
import { serializeHost } from "../src/codec.mjs";
import fixture from "./market-browser/cases.json";
import schema from "../src/generated/schemas/market_context_state.json";
vi.mock(
  "../registry/primitives/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("../registry/primitives/finstack-chart/finstack-chart")
    >()),
    FinstackChart: () => <div>Native chart</div>,
  }),
);
afterEach(cleanup);
const market = fixture.supplemental as unknown as MarketContextStateWire;
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
it("retains the fresh full credit-desk calibration and native canonical supplemental fixture", () => {
  const source = readFileSync(
    resolve(dirname(fileURLToPath(import.meta.url)), "../..", fixture.source),
    "utf8",
  );
  expect(createHash("sha256").update(source).digest("hex")).toBe(
    fixture.sha256,
  );
  expect(native.calibrate(source).result.final_market).toEqual(
    fixture.calibrated,
  );
  const handle = new native.Market(JSON.stringify(market));
  try {
    expect(JSON.parse(handle.toJson())).toEqual(market);
  } finally {
    handle.free();
  }
});
it("navigates every supplied leaf with exact displayed and copied values", async () => {
  const copy = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: copy },
  });
  const select = vi.fn(),
    rendered = render(<MarketContextBrowser state={market} />);
  for (const state of [
    fixture.calibrated as unknown as MarketContextStateWire,
    market,
  ]) {
    const entries = flattenMarket(marketTree(state));
    expect(new Set(entries.map((e) => e.key)).size).toBe(entries.length);
    expect(marketTree(state).map((e) => e.label)).toEqual(
      Object.keys(schema.properties),
    );
    for (const entry of entries.filter((e) => !e.children.length)) {
      const original = entry.path.reduce<unknown>(
        (value, key) => (value as Record<string | number, unknown>)[key],
        state,
      );
      expect(entry.value).toBe(original);
      rendered.rerender(
        <MarketContextBrowser
          state={state}
          link={{ selectedKey: entry.key, select, clear: vi.fn() }}
          renderObject={() => null}
        />,
      );
      const view = screen.getByRole("region", {
        name: "Selected stored value",
      });
      expect(view.querySelector("pre")!.textContent).toBe(
        serializeHost(original),
      );
      fireEvent.click(within(view).getByRole("button", { name: "Copy" }));
      expect(copy).toHaveBeenLastCalledWith(serializeHost(original));
    }
  }
}, 20000);
it("searches literal paths and preserves identified selection across reorder, then reports removal", () => {
  const curve = market.curves.find((c) => c.id === "NEG-RATES")!;
  const entry = flattenMarket(marketTree(market)).find(
    (e) => e.value === curve,
  )!;
  const link = { selectedKey: entry.key, select: vi.fn(), clear: vi.fn() };
  const { rerender } = render(
    <MarketContextBrowser state={market} link={link} />,
  );
  expect(
    screen.getByRole("region", { name: "Selected stored value" }).textContent,
  ).toContain('"min_forward_rate":-1');
  rerender(
    <MarketContextBrowser
      state={{ ...market, curves: [...market.curves].reverse() }}
      link={link}
    />,
  );
  expect(
    screen
      .getByRole("region", { name: "Selected stored value" })
      .querySelector("pre")!.textContent,
  ).toBe(serializeHost(curve));
  fireEvent.change(screen.getByLabelText("Search market fields"), {
    target: { value: "USD~1CSA" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Inspect /collateral/USD~1CSA" }),
  );
  expect(link.select).toHaveBeenLastCalledWith(
    JSON.stringify(["collateral", "USD/CSA"]),
  );
  expect(marketPointer(["a.b", "x~/y", 0])).toBe("/a.b/x~0~1y/0");
  rerender(
    <MarketContextBrowser
      state={{ ...market, curves: market.curves.filter((c) => c !== curve) }}
      link={link}
    />,
  );
  expect(
    screen.getByText("Selected field is no longer available"),
  ).toBeTruthy();
  expect(link.clear).not.toHaveBeenCalled();
});
it("shows absent generated roots as unavailable and preserves null and empty stored values", () => {
  const state = {
    curves: [],
    fx: null,
    prices: {},
  } as unknown as MarketContextStateWire;
  render(<MarketContextBrowser state={state} />);
  fireEvent.click(screen.getByRole("button", { name: "Inspect /hierarchy" }));
  expect(screen.getByText("Field unavailable in supplied state")).toBeTruthy();
  for (const [field, value] of [
    ["curves", "[]"],
    ["fx", "null"],
    ["prices", "{}"],
  ]) {
    fireEvent.click(screen.getByRole("button", { name: `Inspect /${field}` }));
    expect(
      screen
        .getByRole("region", { name: "Selected stored value" })
        .querySelector("pre")!.textContent,
    ).toBe(value);
  }
});
it("preserves FX source, direction, date, policy and unavailable diagonals without resolution", () => {
  render(<FxMatrixGrid state={market.fx} />);
  const rows = (source: string) =>
    Array.from(
      screen.getByRole("table", { name: source }).querySelectorAll("tbody tr"),
      (row) =>
        Array.from(row.querySelectorAll("td"), (cell) => cell.textContent),
    );
  expect(rows("quotes")).toEqual([
    ["EUR", "Unavailable", "1.1 · Undated", "Unavailable"],
    ["USD", "Unavailable", "Unavailable", "Unavailable"],
    ["JPY", "Unavailable", "Unavailable", "Unavailable"],
  ]);
  expect(rows("provider_quotes")[0]![2]).toBe("1.12 · Undated");
  expect(rows("provider_quotes")[2]![2]).toBe("0.007 · Undated");
  expect(rows("pinned_quotes")[0]![2]).toBe(
    market
      .fx!.pinned_quotes.map((q) => `${q[4]} · ${q[2]} · ${q[3]}`)
      .join("; "),
  );
  expect(
    screen
      .getByRole("region", { name: "Complete stored FX state" })
      .querySelector("pre")!.textContent,
  ).toBe(serializeHost(market.fx));
});
