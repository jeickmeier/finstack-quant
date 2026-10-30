// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import type { HorizonReport } from "finstack-quant-wasm";
import { horizonReturnOptions } from "@/hooks/scenarios/use-horizon-return/use-horizon-return";
import { HorizonReturnReport } from "@/components/finstack/scenarios/components/horizon-return-report/horizon-return-report";
import { unwrap, type HorizonReturnRequest } from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./horizon/cases.json";
import bond from "../../src/fixtures/results/bond.json";
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
// Figures are drawn by the shared chart in the browser gates; here each one
// is a labelled placeholder.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
afterEach(cleanup);
const base = bond.request;
const entry = (name: string) =>
  fixture.cases.find((candidate) => candidate.name === name)!;
const request = (
  name: string,
): HorizonReturnRequest & { method: HorizonReturnRequest["method"] } => {
  const { scenarioJson, method, calendarId } = entry(name);
  return {
    instrumentJson: base.instrumentJson,
    marketJson: base.marketJson,
    asOf: base.asOf,
    scenarioJson,
    method: method as HorizonReturnRequest["method"],
    calendarId,
  };
};
const report = (name: string) => entry(name).report as unknown as HorizonReport;
// Horizon returns are regenerated and compared by `mise run ui-fixtures --check`
// (tests/scenarios/horizon/generate.mjs).
it("pins the JSON inputs the horizon fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/scenarios/horizon/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it.each(
  fixture.cases.filter((candidate) => "report" in candidate).map((c) => c.name),
)(
  "computes the %s horizon return through the real worker exactly as the native fixture",
  async (name) => {
    const built = unwrap(
      await harness.proxy.buildMarketScenario({
        id: entry(name).draft.id,
        shifts: entry(name).draft.shifts.map((shift) => ({
          curveKind: shift.curveKind as "discount",
          curveIds: [shift.curveId],
          bp: Number(shift.bp),
        })),
        roll: entry(name).draft.roll && {
          period: entry(name).draft.roll!.period,
          rollMode: null,
        },
        hazardBumpMode: null,
      }),
    );
    expect(built).toBe(entry(name).scenarioJson);
    const result = unwrap(await harness.proxy.horizonReturn(request(name)));
    // The fixture stores JSON, where Rust's -0 residual_pct reads as 0.
    expect(JSON.parse(JSON.stringify(result.report))).toEqual(
      entry(name).report,
    );
    expect(result.explain).toBe(entry(name).explain);
  },
  budget(15000),
);
it("pins Rust's horizon figures and leaves the annualized return undefined without a roll", () => {
  const rolled = report("roll-3m-50bp");
  expect(rolled.horizon_days).toBe(90);
  expect(rolled.summary.total_return).toBeCloseTo(0.008994543549971132, 15);
  expect(rolled.summary.annualized_return).toBeCloseTo(
    0.036982234305995565,
    15,
  );
  // Parallel attribution returns no waterfall steps.
  expect(rolled.attribution.waterfall_steps).toBeUndefined();
  expect(rolled.attribution.meta.method).toBe("parallel");
  const instant = report("usd-ois-50bp");
  expect(instant.horizon_days ?? null).toBeNull();
  expect(instant.summary.annualized_return ?? null).toBeNull();
  expect(report("roll-3m-50bp-taylor").attribution.meta.method).toHaveProperty(
    "taylor",
  );
});
it("returns Rust's metrics_based note and an unknown calendar's native error unchanged", async () => {
  // The bond's hazard curve has no calibration recipe: Rust drops the
  // quote-space credit metrics with a note instead of failing.
  expect(
    report("roll-3m-50bp-metrics-based").attribution.meta.notes.some((note) =>
      note.includes("calibration recipe"),
    ),
  ).toBe(true);
  const calendar = await harness.proxy.horizonReturn({
    ...request("roll-3m-50bp"),
    calendarId: "no-such-calendar",
  });
  expect(calendar.ok).toBe(false);
  if (!calendar.ok) expect(calendar.error.kind).toBe("not_found");
});
it("keys the horizon return on the worker session and the complete snapshotted request", () => {
  const supplied = request("roll-3m-50bp-taylor");
  const options = horizonReturnOptions(null, 5, supplied);
  expect(options.queryKey).toEqual(["finstack", 5, "horizonReturn", supplied]);
  expect(options.queryKey[3]).not.toBe(supplied);
  expect(horizonReturnOptions(null, 5, null).enabled).toBe(false);
});
it("shows Rust's summary, contributions, explain text and attribution as returned", () => {
  const rolled = report("roll-3m-50bp");
  render(
    <HorizonReturnReport
      report={rolled}
      explain={entry("roll-3m-50bp").explain}
      density="comfortable"
    />,
  );
  expect(
    screen.getByText("Total return (decimal)").nextSibling!.textContent,
  ).toBe(String(rolled.summary.total_return));
  expect(
    screen.getByText("Annualized return (decimal)").nextSibling!.textContent,
  ).toBe(String(rolled.summary.annualized_return));
  expect(
    screen.getByText("Horizon (calendar days)").nextSibling!.textContent,
  ).toBe("90");
  expect(screen.getByLabelText("Initial value").title).toBe(
    rolled.initial_value.amount,
  );
  const contributions = screen.getByRole("table", {
    name: "Factor contributions",
  });
  const rows = (
    within(contributions).getAllByRole("row") as HTMLTableRowElement[]
  ).slice(1);
  // Rust's key order, with every returned value as returned.
  expect(rows.map((row) => row.cells[1]!.textContent)).toEqual(
    Object.values(rolled.summary.factor_contributions).map(String),
  );
  expect(rows[0]!.cells[0]!.textContent).toBe("Carry");
  expect(
    within(screen.getByRole("region", { name: "Explain" })).getByText(
      /Annualized: 3\.6982%/,
    ),
  ).toBeTruthy();
  expect(screen.getByRole("region", { name: "P&L attribution" })).toBeTruthy();
  // The scenario report renders only once its disclosure opens.
  expect(
    screen.queryByRole("region", { name: "Application report" }),
  ).toBeNull();
  const disclosure = screen
    .getByText("Scenario application report")
    .closest("details")!;
  disclosure.open = true;
  fireEvent(disclosure, new Event("toggle"));
  const json = screen.getByRole("region", { name: "Application report" });
  expect(JSON.parse(json.querySelector("code")!.textContent!)).toEqual(
    rolled.scenario_report,
  );
});
it("shows an undefined annualized return as not defined, never 0", () => {
  render(<HorizonReturnReport report={report("usd-ois-50bp")} />);
  expect(
    screen.getByText("Annualized return (decimal)").nextSibling!.textContent,
  ).toBe("not defined");
  expect(
    screen.getByText("Horizon (calendar days)").nextSibling!.textContent,
  ).toBe("no time roll");
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<HorizonReturnReport report={null} />);
  expect(screen.getByRole("status").textContent).toBe("No horizon return");
  rerender(<HorizonReturnReport report={null} loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Computing horizon return…",
  );
  rerender(
    <HorizonReturnReport
      report={report("roll-3m-50bp")}
      error="Requested item not found: no-such-calendar"
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain(
    "Requested item not found: no-such-calendar",
  );
});
