// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { it, expect, afterEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { createChartRuntime } from "@tanstack/charts";
import type { CalibrationReport as NativeReport } from "finstack-quant-wasm";
import type { CalibrationWire } from "../src/generated/types/calibration";
import { CalibrationReport } from "../registry/components/calibration-report/calibration-report";
import {
  residualDefinition,
  residualPanels,
  type ResidualPoint,
} from "../registry/components/calibration-fit-chart/calibration-fit-chart";
import { serializeHost } from "../src/codec.mjs";
import failure from "./calibration/failure.json";
import cases from "./calibration/cases.json";
import inventory from "../src/generated/fixtures.json";
afterEach(cleanup);
const reports = cases as unknown as {
  source: string;
  sha256: string;
  variant?: string;
  input: CalibrationWire;
  result: {
    result: {
      report: NativeReport;
      step_reports: Record<string, NativeReport>;
    };
  };
}[];
it("covers every discovered calibration envelope and original source digest", () => {
  expect(reports.filter((c) => !c.variant).map((c) => c.source)).toEqual(
    inventory
      .filter((f) => f.kind === "calibration-input")
      .map((f) => f.source),
  );
  for (const c of reports) {
    const source = readFileSync(
      resolve(dirname(fileURLToPath(import.meta.url)), "../..", c.source),
    );
    expect(createHash("sha256").update(source).digest("hex")).toBe(c.sha256);
  }
});
it("plots every native signed residual by its original step/quote label with a zero target", () => {
  let count = 0;
  for (const fixture of reports)
    for (const [stepId, report] of Object.entries(
      fixture.result.result.step_reports,
    )) {
      const panels = residualPanels(stepId, report, fixture.input.market_data),
        points = panels.flatMap((p) => p.points);
      expect(
        Object.fromEntries(points.map((p) => [p.quoteId, p.residual])),
      ).toEqual(report.residuals);
      for (const panel of panels) {
        const runtime = createChartRuntime<ResidualPoint, string, number>();
        try {
          const scene = runtime.render(residualDefinition(panel, {}), {
            width: 900,
            height: 550,
          });
          const marks = scene.points;
          expect(marks.map((p) => [p.xValue, p.yValue])).toEqual(
            panel.points.map((p) => [p.quoteId, p.residual]),
          );
          for (const p of marks) {
            expect(p.datum.report).toBe(report);
            expect(p.datum.stepId).toBe(stepId);
          }
          // The decorative zero rule contributes the zero domain endpoint without becoming a hover point.
          expect(scene.points.every((p) => p.datum.report === report)).toBe(
            true,
          );
          const y = scene.scales.y!;
          expect(Math.min(...(y.domain as number[]))).toBeLessThanOrEqual(0);
          expect(Math.max(...(y.domain as number[]))).toBeGreaterThanOrEqual(0);
          count += marks.length;
        } finally {
          runtime.destroy();
        }
      }
    }
  expect(count).toBeGreaterThan(100);
});
it("separates deposit/swap and unlike steps and isolates missing input identities", () => {
  const report = reports[0]!.result.result.step_reports["USD-OIS"]!;
  const groups = residualPanels(
    "USD-OIS",
    report,
    reports[0]!.input.market_data,
  );
  expect(groups.map((p) => p.group).sort()).toEqual([
    "rate_quote / deposit",
    "rate_quote / swap",
  ]);
  expect(
    groups.every(
      (p) => p.units === "Solver units (quote convention unavailable)",
    ),
  ).toBe(true);
  expect(
    residualPanels("USD-OIS", report).every((p) => p.points.length === 1),
  ).toBe(true);
  const plan = reports[0]!.result.result.report;
  expect(residualPanels("Plan", plan)[0]!.units).toBe(
    plan.metadata.residual_units,
  );
  const duplicate = [
    reports[0]!.input.market_data![0]!,
    reports[0]!.input.market_data![0]!,
  ];
  expect(() => residualPanels("USD-OIS", report, duplicate)).toThrow(
    "Duplicate",
  );
});
it("renders exact plan and step reports, unavailable diagnostics and native zero-target representation", () => {
  for (const fixture of reports)
    for (const [stepId, report] of [
      ["Plan", fixture.result.result.report],
      ...Object.entries(fixture.result.result.step_reports),
    ] as const) {
      const mounted = render(
        <CalibrationReport stepId={stepId} report={report} />,
      );
      expect(
        screen
          .getByRole("region", { name: `${stepId} complete report` })
          .querySelector("pre")!.textContent,
      ).toBe(serializeHost(report));
      if (report.diagnostics) {
        const rows = Array.from(
          screen
            .getByRole("table", {
              name: `${stepId} native per-quote diagnostics`,
            })
            .querySelectorAll("tbody tr"),
          (row) =>
            Array.from(row.querySelectorAll("td"), (cell) => cell.textContent),
        );
        expect(rows).toEqual(
          report.diagnostics.per_quote.map((q) =>
            [
              q.quote_label,
              q.target_value,
              q.fitted_value,
              q.residual,
              q.sensitivity,
            ].map(String),
          ),
        );
        expect(
          report.diagnostics.per_quote.every(
            (q) => q.target_value === 0 && q.fitted_value === q.residual,
          ),
        ).toBe(true);
      } else expect(screen.getByText("Diagnostics unavailable")).toBeTruthy();
      mounted.unmount();
    }
}, 15000);

it("preserves actual native structured failures with available and unavailable solver diagnostics", () => {
  const { rerender } = render(
    <CalibrationReport stepId={failure.step_id} error={failure} />,
  );
  expect(screen.getByRole("alert").textContent).toBe(failure.message);
  expect(
    screen
      .getByRole("region", { name: `${failure.step_id} structured failure` })
      .querySelector("pre")!.textContent,
  ).toBe(serializeHost(failure));
  expect(
    screen
      .getByRole("region", { name: `${failure.step_id} solver diagnostics` })
      .querySelector("pre")!.textContent,
  ).toBe(serializeHost(failure.solver_diagnostics));
  rerender(
    <CalibrationReport
      stepId="Unavailable"
      error={{
        message: "Unavailable diagnostics",
        solver_diagnostics: undefined,
      }}
    />,
  );
  expect(screen.getByText("Solver diagnostics unavailable")).toBeTruthy();
});
