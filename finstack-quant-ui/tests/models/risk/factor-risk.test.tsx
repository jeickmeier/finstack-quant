// @vitest-environment jsdom
import {
  afterAll,
  afterEach,
  beforeAll,
  beforeEach,
  expect,
  it,
  vi,
} from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  factorRiskOptions,
  useFactorRisk,
  type FactorRiskRequest,
} from "@/hooks/models/use-factor-risk/use-factor-risk";
import {
  PortfolioRiskReport,
  type FactorRiskData,
} from "@/components/finstack/models/components/portfolio-risk-report/portfolio-risk-report";
import { unwrap, type WorkerApi } from "@/workers/finstack-contract";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
const mock = vi.hoisted(() => ({
  createClient: vi.fn(),
  calls: [] as string[],
}));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
let harness: Awaited<ReturnType<typeof startWorker>>;
const proxy = () => harness.proxy as unknown as WorkerApi;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.calls.length = 0;
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) => {
      mock.calls.push(method);
      if (method === "initialize") return unwrap(await proxy().initialize());
      if (method === "computeFactorSensitivities")
        return unwrap(
          await proxy().computeFactorSensitivities(request as never),
        );
      if (method === "decomposeFactors")
        return unwrap(await proxy().decomposeFactors(request as never));
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const request = fixture.factor.request as FactorRiskRequest;
const report = fixture.factor.report as unknown as FactorRiskData;
/** Compare through JSON, as the fixture was written (Rust may return -0). */
const json = (value: unknown) => JSON.parse(JSON.stringify(value));
// Regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/risk/generate.mjs).
it(
  "returns the fixture sensitivities and decomposition through the real worker",
  async () => {
    const sensitivities = unwrap(
      await proxy().computeFactorSensitivities(request),
    );
    expect(json(sensitivities)).toEqual(report.sensitivities);
    const decomposition = unwrap(
      await proxy().decomposeFactors({
        sensitivitiesJson: JSON.stringify(sensitivities),
        covarianceJson: request.covarianceJson,
      }),
    );
    expect(json(decomposition)).toEqual(report.decomposition);
  },
  budget(15000),
);
it("keeps the host-parity probe: one factor carries all the variance", () => {
  // Rows follow the book's positions, columns the factor definitions.
  expect(report.sensitivities.position_ids).toEqual(["POS-0", "POS-1"]);
  expect(report.sensitivities.factor_ids).toEqual(["usd_rates"]);
  expect(report.decomposition.measure).toBe("variance");
  expect(report.decomposition.total_risk).toBeCloseTo(2.2135, 4);
  expect(report.decomposition.factor_contributions[0]!.relative_risk).toBe(1);
});
it("passes the risk measure through and keeps its returned sign", async () => {
  const sensitivitiesJson = JSON.stringify(report.sensitivities);
  const tail = unwrap(
    await proxy().decomposeFactors({
      sensitivitiesJson,
      covarianceJson: request.covarianceJson,
      riskMeasureJson: JSON.stringify({ var: { confidence: 0.99 } }),
    }),
  );
  expect(tail.measure).toEqual({ var: { confidence: 0.99 } });
  expect(tail.total_risk).toBeLessThan(0);
});
it("returns Rust's rejection of unknown fields and misaligned factors", async () => {
  const factors = JSON.parse(request.factorsJson) as object[];
  const unknown = await proxy().computeFactorSensitivities({
    ...request,
    factorsJson: JSON.stringify([{ ...factors[0], extra: 1 }]),
  });
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) expect(unknown.error.kind).toBe("validation");
  const misaligned = await proxy().decomposeFactors({
    sensitivitiesJson: JSON.stringify(report.sensitivities),
    covarianceJson: JSON.stringify({
      factor_ids: ["other"],
      n: 1,
      data: [0.0001],
    }),
  });
  expect(misaligned.ok).toBe(false);
});
it("keys the chain on the worker session and the complete request", () => {
  const options = factorRiskOptions(null, 3, request);
  expect(options.queryKey).toEqual(["finstack", 3, "factorRisk", request]);
  expect(options.queryKey[3]).not.toBe(request);
  expect(factorRiskOptions(null, 3, null).enabled).toBe(false);
});
it("shows the factor section alone when no position report is supplied", () => {
  render(<PortfolioRiskReport report={null} factors={report} />);
  const section = screen.getByRole("region", { name: "Factor contributions" });
  expect(
    within(
      within(section).getByRole("region", { name: "Total risk" }),
    ).getByText(String(report.decomposition.total_risk)),
  ).toBeTruthy();
  expect(within(section).getByText("variance")).toBeTruthy();
  const sensitivities = within(section).getByRole("table", {
    name: "Factor sensitivities (USD)",
  });
  expect(
    within(sensitivities).getByText(String(report.sensitivities.data[1]![0])),
  ).toBeTruthy();
  expect(screen.queryByRole("region", { name: "Component VaR" })).toBeNull();
  cleanup();
  render(<PortfolioRiskReport report={null} factorsError="unknown factor" />);
  expect(screen.getByRole("alert").textContent).toBe("unknown factor");
});
function Connected({ request }: { request: FactorRiskRequest }) {
  const factors = useFactorRisk(request);
  return (
    <PortfolioRiskReport
      report={null}
      factors={(factors.data as FactorRiskData | undefined) ?? null}
      factorsError={factors.error?.message}
      loading={factors.isFetching}
    />
  );
}
it("chains the sensitivities into the decomposition through the hook, then shows the first native error", async () => {
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        screen.getByRole("region", { name: "Factor contributions" }),
      ).toBeTruthy(),
    { timeout: budget(5000) },
  );
  expect(mock.calls.filter((call) => call !== "initialize")).toEqual([
    "computeFactorSensitivities",
    "decomposeFactors",
  ]);
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected request={{ ...request, asOf: "not-a-date" }} />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
