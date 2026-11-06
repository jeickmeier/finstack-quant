// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  ExposureReport,
  type ExposureReportProps,
  type PortfolioPrimitiveExposureReport,
  type SensitivityMatrixJson,
} from "@/components/finstack/portfolio/components/exposure-report/exposure-report";
import {
  factorSensitivitiesOptions,
  primitiveExposuresOptions,
} from "@/hooks/portfolio/use-exposure-report/use-exposure-report";
import {
  PortfolioWorkbench,
  type PortfolioWorkbenchProps,
} from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import fixture from "./exposures/cases.json";
import workbench from "./workbench/cases.json";
import pricingMarket from "../valuations/instruments/pricing-market.json";

vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
// Figures are drawn by the shared chart in the browser gates.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
const harness = useWorkerHarness("any");
const proxy = () => harness.proxy as unknown as WorkerApi;
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const report = fixture.report as PortfolioPrimitiveExposureReport;
const unmeasured = fixture.unmeasured as PortfolioPrimitiveExposureReport;
const sensitivities = fixture.sensitivities as SensitivityMatrixJson;
const { inputs, asOf } = fixture;
const book = {
  specJson: workbench.specJson,
  marketJson: JSON.stringify(pricingMarket),
};
const plain = (value: unknown) => JSON.parse(JSON.stringify(value));
function view(overrides: Partial<ExposureReportProps> = {}) {
  return render(
    <ExposureReport
      report={report}
      sensitivities={sensitivities}
      {...overrides}
    />,
  );
}
const region = (name: string) => within(screen.getByRole("region", { name }));
const shows = (scope: ReturnType<typeof region>, value: number | string) =>
  expect(scope.getAllByTitle(String(value)).length).toBeGreaterThan(0);

it(
  "the worker returns the native exposures of the book with and without metrics",
  async () => {
    expect(
      plain(
        unwrap(
          await proxy().primitiveExposures({
            ...book,
            metrics: inputs.metrics,
          }),
        ),
      ),
    ).toEqual(report);
    expect(
      plain(unwrap(await proxy().primitiveExposures({ ...book, metrics: [] }))),
    ).toEqual(unmeasured);
  },
  budget(30000),
);
it(
  "the worker returns the native sensitivity matrix of the host's factors",
  async () => {
    const matrix = unwrap(
      await proxy().computeFactorSensitivities({
        ...book,
        factorsJson: inputs.factorsJson,
        asOf,
      }),
    );
    expect(plain(matrix)).toEqual(sensitivities);
  },
  budget(30000),
);
it.each(["nonAdditive", "notApplicable"] as const)(
  "the worker returns the native exposure error of %s",
  async (name) => {
    const entry = fixture.errors[name];
    const outcome = await proxy().primitiveExposures({
      ...book,
      metrics: entry.request.metrics,
    });
    expect(outcome.ok).toBe(false);
    if (outcome.ok) return;
    expect(outcome.error.message).toBe(entry.error.message);
    expect(outcome.error.kind).toBe(entry.error.kind);
  },
  budget(30000),
);
it(
  "the worker returns the native error of a factor on a curve the market lacks",
  async () => {
    const entry = fixture.errors.missingCurve;
    const outcome = await proxy().computeFactorSensitivities({
      ...book,
      factorsJson: entry.request.factorsJson,
      asOf,
    });
    expect(outcome.ok).toBe(false);
    if (outcome.ok) return;
    expect(outcome.error.message).toBe(entry.error.message);
    expect(outcome.error.kind).toBe(entry.error.kind);
  },
  budget(30000),
);
it("Rust returns both net and gross, and they differ for a short risk", () => {
  const deposit = report.aggregates.find(
    (row) => row.instrument_id === "DEP-USD-6M",
  )!;
  expect(deposit.net_measures.dv01).toBeLessThan(0);
  expect(deposit.gross_measures.dv01).toBeGreaterThan(0);
});
it("shows each primitive's returned net and gross quantity and value in returned order", () => {
  view();
  const section = region("Net and gross by primitive");
  const rows = within(
    section.getByRole("table", { name: "Net and gross by primitive" }),
  ).getAllByRole("row");
  expect(rows).toHaveLength(report.aggregates.length + 1);
  report.aggregates.forEach((aggregate, index) => {
    const row = within(rows[index + 1]!);
    expect(row.getByText(aggregate.instrument_id)).toBeTruthy();
    expect(row.getByText(aggregate.instrument_type)).toBeTruthy();
    shows(row, aggregate.net_quantity);
    shows(row, aggregate.gross_quantity);
    shows(row, aggregate.net_value.amount);
    shows(row, aggregate.gross_value.amount);
  });
  expect(
    section.getByRole("img", { name: "Net and gross value by primitive" }),
  ).toBeTruthy();
  expect(section.getByText(/in USD/)).toBeTruthy();
});
it("shows every returned metric's net and gross amount per primitive", () => {
  view();
  const rows = within(
    region("Risk by primitive").getByRole("table", {
      name: "Risk by primitive",
    }),
  ).getAllByRole("row");
  const expected = report.aggregates.flatMap((aggregate) =>
    Object.keys(aggregate.net_measures).map((metric) => ({
      aggregate,
      metric,
    })),
  );
  expect(rows).toHaveLength(expected.length + 1);
  expected.forEach(({ aggregate, metric }, index) => {
    const row = within(rows[index + 1]!);
    expect(row.getByText(aggregate.instrument_id)).toBeTruthy();
    expect(row.getByText(metric)).toBeTruthy();
    shows(row, aggregate.net_measures[metric]!);
    shows(row, aggregate.gross_measures[metric]!);
  });
});
it("says no metric was requested when Rust returns no measures", () => {
  view({ report: unmeasured });
  expect(region("Risk by primitive").getByRole("status").textContent).toBe(
    "No risk metrics requested",
  );
});
it("shows every position's primitive paths as returned", () => {
  view();
  const rows = within(
    region("Primitive paths").getByRole("table", { name: "Primitive paths" }),
  ).getAllByRole("row");
  expect(rows).toHaveLength(report.paths.length + 1);
  report.paths.forEach((path, index) => {
    const row = within(rows[index + 1]!);
    // A direct instrument's path is its own position id.
    expect(row.getAllByText(path.position_id).length).toBeGreaterThan(0);
    expect(row.getAllByText(path.path.join(" › ")).length).toBeGreaterThan(0);
    shows(row, path.quantity);
    shows(row, path.value.amount);
  });
});
it("shows the supplied sensitivity matrix, its error, and leaves it out when none is supplied", () => {
  view();
  const section = region("Factor sensitivities");
  const table = within(
    section.getByRole("table", {
      name: `Factor sensitivities (${sensitivities.base_currency})`,
    }),
  );
  expect(table.getByText(sensitivities.factor_ids[0]!)).toBeTruthy();
  const rows = table.getAllByRole("row");
  expect(rows).toHaveLength(sensitivities.position_ids.length + 1);
  sensitivities.position_ids.forEach((position, index) => {
    const row = within(rows[index + 1]!);
    expect(row.getByText(position)).toBeTruthy();
    for (const value of sensitivities.data[index]!) shows(row, value);
  });
  cleanup();
  view({ sensitivities: undefined });
  expect(
    screen.queryByRole("region", { name: "Factor sensitivities" }),
  ).toBeNull();
  cleanup();
  const { message } = fixture.errors.missingCurve.error;
  view({ sensitivities: null, sensitivitiesError: message });
  expect(region("Factor sensitivities").getByRole("alert").textContent).toBe(
    message,
  );
});
it("shows the native error, loading and no result", () => {
  const { message } = fixture.errors.nonAdditive.error;
  view({ report: null, error: message, sensitivities: undefined });
  expect(screen.getByRole("alert").textContent).toBe(message);
  cleanup();
  view({ report: null, loading: true, sensitivities: undefined });
  expect(screen.getByRole("status").textContent).toBe("Decomposing exposures…");
  cleanup();
  view({ report: null, sensitivities: undefined });
  expect(screen.getByRole("status").textContent).toBe("No exposure report");
});
it("the hooks are disabled without a request and key on the whole request", () => {
  const client = {} as FinstackClient;
  const request = { ...book, metrics: inputs.metrics };
  expect(primitiveExposuresOptions(client, 1, null).enabled).toBe(false);
  expect(primitiveExposuresOptions(client, 1, request).queryKey).toEqual([
    "finstack",
    1,
    "primitiveExposures",
    request,
  ]);
  const factors = { ...book, factorsJson: inputs.factorsJson, asOf };
  expect(factorSensitivitiesOptions(client, 1, null).enabled).toBe(false);
  expect(factorSensitivitiesOptions(client, 2, factors).queryKey).toEqual([
    "finstack",
    2,
    "computeFactorSensitivities",
    factors,
  ]);
});
const props: PortfolioWorkbenchProps = {
  defaultSpecJson: workbench.specJson,
  marketJson: JSON.stringify(pricingMarket),
  asOf,
  exposures: inputs,
};
function renderWorkbench(overrides: Partial<PortfolioWorkbenchProps> = {}) {
  return render(
    <FinstackQueryProvider>
      <PortfolioWorkbench {...props} {...overrides} />
    </FinstackQueryProvider>,
  );
}
it(
  "decomposes the book and its factor matrix once when the Risk tab opens",
  async () => {
    const user = userEvent.setup();
    renderWorkbench();
    expect(harness.methods()).not.toContain("primitiveExposures");
    await user.click(screen.getByRole("tab", { name: "Risk" }));
    const exposures = await screen.findByRole(
      "table",
      { name: "Net and gross by primitive" },
      { timeout: budget(10000) },
    );
    shows(within(exposures), report.aggregates[1]!.net_value.amount);
    expect(
      await screen.findByRole(
        "table",
        { name: `Factor sensitivities (${sensitivities.base_currency})` },
        { timeout: budget(10000) },
      ),
    ).toBeTruthy();
    expect(screen.queryAllByRole("alert")).toEqual([]);
    expect(
      harness.methods().filter((name) => name === "primitiveExposures"),
    ).toHaveLength(1);
    expect(
      harness.methods().filter((name) => name === "computeFactorSensitivities"),
    ).toHaveLength(1);
  },
  budget(30000),
);
it(
  "decomposes quantity and value only, with no matrix, when the host supplies no exposure inputs",
  async () => {
    const user = userEvent.setup();
    renderWorkbench({ exposures: undefined });
    await user.click(screen.getByRole("tab", { name: "Risk" }));
    const exposures = await screen.findByRole(
      "region",
      { name: "Primitive exposures" },
      { timeout: budget(10000) },
    );
    expect(
      (
        await within(exposures).findAllByText(
          "No risk metrics requested",
          {},
          { timeout: budget(10000) },
        )
      ).length,
    ).toBe(1);
    expect(
      screen.queryByRole("region", { name: "Factor sensitivities" }),
    ).toBeNull();
    expect(harness.methods()).not.toContain("computeFactorSensitivities");
  },
  budget(30000),
);
