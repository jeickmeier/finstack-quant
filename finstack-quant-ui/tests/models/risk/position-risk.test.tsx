// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  positionRiskOptions,
  usePositionRisk,
  type PositionRisk,
  type PositionRiskRequest,
} from "@/hooks/models/use-position-risk/use-position-risk";
import {
  PortfolioRiskReport,
  type PositionRiskData,
} from "@/components/finstack/models/components/portfolio-risk-report/portfolio-risk-report";
import { unwrap } from "@/workers/finstack-contract";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
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
const harness = useWorkerHarness(["positionRisk"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
type Case = "parametric" | "historical";
const entry = (name: Case) => ({
  request: fixture[name].request as PositionRiskRequest,
  report: fixture[name].report as unknown as PositionRisk,
});
/** Compare through JSON, as the fixture was written (Rust may return -0). */
const json = (value: unknown) => JSON.parse(JSON.stringify(value));
// Reports are regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/risk/generate.mjs).
it.each(["parametric", "historical"] as const)(
  "returns the %s fixture report through the real worker",
  async (name) => {
    const { request, report } = entry(name);
    expect(json(unwrap(await harness.proxy.positionRisk(request)))).toEqual(
      report,
    );
  },
  budget(15000),
);
it("keeps the returned loss convention, method stamps and budget inputs", () => {
  const parametric = entry("parametric").report;
  const { decomposition } = parametric;
  expect(decomposition.method).toBe("parametric");
  expect(decomposition.portfolio_var).toBeLessThan(0);
  expect(decomposition.euler_residual).not.toBeNull();
  // The budget's actual component VaR is positionComponentVar of each id.
  if (!parametric.budget?.ok) throw new Error("fixture budget");
  expect(
    parametric.budget.value.positions.map((row) => row.actual_component_var),
  ).toEqual(decomposition.var_contributions.map((row) => row.component_var));
  expect(parametric.stress).toBeNull();
  const historical = entry("historical").report;
  expect(historical.decomposition.method).toBe("historical");
  expect(historical.decomposition.euler_residual).toBeNull();
  expect(
    historical.decomposition.var_contributions[0]!.marginal_var,
  ).toBeNull();
  if (!historical.stress?.ok) throw new Error("fixture stress");
  // Rust's largest-driver-first order, not the request's position order.
  expect(
    historical.stress.value.position_contributions.map(
      (row) => row.position_id,
    ),
  ).toEqual(["Credit", "Equity", "Rates"]);
});
it("leaves omitted options unset so Rust applies its defaults", async () => {
  const { request } = entry("parametric");
  if (request.source.kind !== "parametric") throw new Error("fixture source");
  const report = unwrap(
    await harness.proxy.positionRisk({
      positionIds: request.positionIds,
      source: {
        kind: "parametric",
        weights: request.source.weights,
        covariance: request.source.covariance,
      },
    }),
  );
  expect(report.decomposition.confidence).toBe(0.95);
  expect(report.decomposition.var_contributions[0]!.incremental_var).toBeNull();
  expect(report.budget).toBeNull();
  expect(report.stress).toBeNull();
});
it("keeps a rejected budget's native error beside the decomposition", async () => {
  const { request, report } = entry("parametric");
  const rejected = unwrap(
    await harness.proxy.positionRisk({
      ...request,
      budget: { targets: [0.5, 0.5, 0.5] },
    }),
  );
  expect(json(rejected.decomposition)).toEqual(report.decomposition);
  expect(rejected.budget?.ok).toBe(false);
  if (rejected.budget && !rejected.budget.ok)
    expect(rejected.budget.error.kind).toBe("validation");
});
it("returns the native error of a rejected decomposition and an unknown source", async () => {
  const { request } = entry("parametric");
  if (request.source.kind !== "parametric") throw new Error("fixture source");
  const failed = await harness.proxy.positionRisk({
    ...request,
    source: { ...request.source, weights: [0.5, 0.5] },
  });
  expect(failed.ok).toBe(false);
  if (!failed.ok) expect(failed.error.kind).toBe("validation");
  const unknown = await harness.proxy.positionRisk({
    ...request,
    source: { kind: "monte_carlo" } as never,
  });
  expect(unknown.ok).toBe(false);
  if (!unknown.ok)
    expect(unknown.error.message).toBe("Unsupported position risk source");
});
it("keys the query on the worker session and the complete request", () => {
  const { request } = entry("historical");
  const options = positionRiskOptions(null, 3, request);
  expect(options.queryKey.slice(0, 3)).toEqual(["finstack", 3, "positionRisk"]);
  expect(positionRiskOptions(null, 3, null).enabled).toBe(false);
});
const data = (name: Case) =>
  fixture[name].report as unknown as PositionRiskData;
it("shows the returned tiles, contributions, budget and stress attribution", () => {
  const report = data("historical");
  render(<PortfolioRiskReport report={report} />);
  const tile = (name: string) => screen.getByRole("region", { name });
  expect(
    within(tile("Portfolio var")).getByText(
      String(report.decomposition.portfolio_var),
    ),
  ).toBeTruthy();
  expect(within(tile("Euler residual")).getByText("—")).toBeTruthy();
  const contributions = within(
    screen.getByRole("region", { name: "Component VaR" }),
  ).getByRole("table");
  expect(within(contributions).getAllByRole("row")).toHaveLength(4);
  expect(
    screen.getByRole("img", { name: "Component VaR by position" }),
  ).toBeTruthy();
  const budgetRegion = screen.getByRole("region", { name: "Risk budget" });
  if (!report.budget?.ok) throw new Error("fixture budget");
  expect(
    within(
      within(budgetRegion).getByRole("region", { name: "Has breach" }),
    ).getByText(String(report.budget.value.has_breach)),
  ).toBeTruthy();
  const stress = screen.getByRole("region", { name: "Stress attribution" });
  const [contributionsTable, tail] = within(stress).getAllByRole("table");
  const first = within(contributionsTable!).getAllByRole("row")[1]!;
  expect(within(first).getByText("Credit")).toBeTruthy();
  if (!report.stress?.ok) throw new Error("fixture stress");
  expect(within(tail!).getAllByRole("row")).toHaveLength(
    1 + report.stress.value.n_tail_scenarios,
  );
});
it("shows a rejected budget's native error and omits sections Rust did not return", () => {
  const report = data("parametric");
  render(
    <PortfolioRiskReport
      report={{
        ...report,
        budget: { ok: false, error: { message: "targets must sum to one" } },
      }}
    />,
  );
  expect(
    within(screen.getByRole("region", { name: "Risk budget" })).getByRole(
      "alert",
    ).textContent,
  ).toBe("targets must sum to one");
  expect(
    screen.queryByRole("region", { name: "Stress attribution" }),
  ).toBeNull();
});
function Connected({ request }: { request: PositionRiskRequest }) {
  const report = usePositionRisk(request);
  return (
    <PortfolioRiskReport
      report={(report.data as PositionRiskData | undefined) ?? null}
      loading={report.isFetching}
      error={report.error?.message}
    />
  );
}
it("renders the worker report through the hook, then the native error", async () => {
  const { request } = entry("parametric");
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(screen.getByRole("region", { name: "Risk budget" })).toBeTruthy(),
    { timeout: budget(5000) },
  );
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected request={{ ...request, confidence: 0.2 }} />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
