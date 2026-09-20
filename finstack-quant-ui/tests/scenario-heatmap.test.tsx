// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, it, expect, vi } from "vitest";
import { render, screen, fireEvent, cleanup } from "@testing-library/react";
import { createChartRuntime } from "@tanstack/charts";
import type { TrancheScenarioCell } from "finstack-quant-wasm";
import {
  ScenarioHeatmap,
  scenarioCells,
  scenarioPreset,
  scenarioKey,
  scenarioPriceLabel,
} from "../registry/components/scenario-heatmap/scenario-heatmap";
import { ScenarioHeatmapExample } from "../registry/components/scenario-heatmap/example";
import { composeFigure } from "../registry/primitives/finstack-chart/figure";
import tokens from "../registry/theme/finstack-theme/tokens.json";
import { serializeHost } from "../src/codec.mjs";
import fixture from "./scenarios/cases.json";
vi.mock(
  "../registry/primitives/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: () => <div>Native heatmap</div>,
  }),
);
afterEach(cleanup);
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
it("matches direct native cells for original and factor-adjusted tranches on current balance", () => {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  for (const [source, hash] of [
    [fixture.instrumentSource, fixture.instrumentSha256],
    [fixture.marketSource, fixture.marketSha256],
  ])
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(root, source!)))
        .digest("hex"),
    ).toBe(hash);
  const market = readFileSync(resolve(root, fixture.marketSource), "utf8");
  for (const c of fixture.cases)
    expect(
      native.structuredCreditTrancheScenarioTable(
        JSON.stringify(c.instrument),
        fixture.trancheId,
        market,
        fixture.asOf,
        JSON.stringify(fixture.grid),
      ),
    ).toEqual(c.table);
  const adjusted =
    fixture.cases[1]!.instrument.instrument.spec.tranches.tranches[0]!;
  expect(adjusted.current_balance.amount).toBe("50000000");
  expect(adjusted.original_balance.amount).toBe("100000000");
  // Halving both collateral/current face leaves percentage-of-current-face quotes unchanged up to native f64 rounding.
  fixture.cases[1]!.table.cells.forEach((c, i) =>
    expect(c.price).toBeCloseTo(fixture.cases[0]!.table.cells[i]!.price, 10),
  );
});
it("retains original cells, coordinates, severity and par reference through the native heatmap", () => {
  const t = tokens.light;
  for (const c of fixture.cases)
    for (const severity of fixture.grid.severities) {
      const cells = scenarioCells(c.table, severity);
      expect(cells.every((cell) => c.table.cells.includes(cell))).toBe(true);
      const figure = composeFigure(
        {
          ...scenarioPreset({
            table: c.table,
            severity,
            priceDomain: [80, 140],
          }),
          ariaLabel: "Native scenarios",
        },
        {
          theme: {
            foreground: t.foreground,
            background: t.background,
            muted: t["muted-foreground"],
            grid: t.border,
            palette: [t["chart-1"]],
          },
          cellText: { light: t["cell-light"], dark: t["cell-dark"], size: 11 },
          ramps: {
            sequential: Array.from(
              { length: 9 },
              (_, i) => t[`sequential-${i + 1}` as keyof typeof t],
            ),
            diverging: [
              t["diverging-negative"],
              t["diverging-neutral"],
              t["diverging-positive"],
            ],
          },
          fontFamily: "sans-serif",
          titleSize: 16,
          bodySize: 14,
          noteSize: 12,
          spacing: 4,
          measureText: (text, o) => ({
            x: 0,
            y: 0,
            width: text.length * o.fontSize * 0.6,
            height: o.fontSize,
          }),
        },
      );
      const runtime = createChartRuntime<
        TrancheScenarioCell,
        string | number,
        string | number
      >();
      try {
        const scene = runtime.render(figure.definition, {
          width: 1000,
          height: 650,
        });
        expect(scene.points).toHaveLength(cells.length);
        scene.points.forEach((p, i) => {
          expect(p.datum).toBe(cells[i]);
          expect([p.xValue, p.yValue]).toEqual([cells[i]!.cpr, cells[i]!.cdr]);
        });
        expect(scene.colors.domain).toEqual([80, 100, 140]);
      } finally {
        runtime.destroy();
      }
    }
});
it("selects exact supplied severity and preserves complete native JSON without overlay columns", () => {
  const table = fixture.cases[1]!.table,
    onSeverityChange = vi.fn();
  const { rerender } = render(
    <ScenarioHeatmap
      table={table}
      severity={0.2}
      onSeverityChange={onSeverityChange}
      priceDomain={[80, 140]}
    />,
  );
  expect(
    screen.getByText(
      `${scenarioPriceLabel}. Par reference: 100. Severity: 0.2.`,
    ),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "0.6" }));
  expect(onSeverityChange).toHaveBeenLastCalledWith(0.6);
  rerender(
    <ScenarioHeatmap
      table={table}
      severity={0.6}
      onSeverityChange={onSeverityChange}
      priceDomain={[80, 140]}
    />,
  );
  const rows = Array.from(
    screen.getByRole("table").querySelectorAll("tbody tr"),
    (row) => Array.from(row.querySelectorAll("td"), (cell) => cell.textContent),
  );
  expect(rows).toEqual(
    scenarioCells(table, 0.6).map((c) =>
      [c.cpr, c.cdr, c.severity, c.price].map(String),
    ),
  );
  expect(
    screen
      .getByRole("region", {
        name: `${table.tranche_id} complete native scenario table`,
      })
      .querySelector("pre")!.textContent,
  ).toBe(serializeHost(table));
  expect(
    screen.queryByRole("columnheader", { name: /WAL|writedown/i }),
  ).toBeNull();
  rerender(
    <ScenarioHeatmap
      table={table}
      severity={0.9}
      onSeverityChange={onSeverityChange}
      priceDomain={[80, 140]}
    />,
  );
  expect(screen.getByText("Selected severity unavailable")).toBeTruthy();
  expect(scenarioKey(table.tranche_id, table.cells[0]!)).not.toBe(
    scenarioKey("other", table.cells[0]!),
  );
});
it("runs the independent example with no worker and handles an empty result", () => {
  const { rerender } = render(
    <ScenarioHeatmapExample
      table={fixture.cases[0]!.table}
      priceDomain={[80, 140]}
    />,
  );
  expect(screen.getByRole("radio", { name: "0.2" })).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "0.6" }));
  expect(
    screen.getByText(
      `${scenarioPriceLabel}. Par reference: 100. Severity: 0.6.`,
    ),
  ).toBeTruthy();
  rerender(
    <ScenarioHeatmapExample
      key="empty"
      table={{ tranche_id: "empty", cells: [] }}
      priceDomain={[80, 140]}
    />,
  );
  expect(screen.getByText("No returned scenario cells")).toBeTruthy();
});
