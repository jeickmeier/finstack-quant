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
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  mertonReportOptions,
  useMertonReport,
  type MertonReport,
  type MertonReportRequest,
} from "@/hooks/models/use-merton-report/use-merton-report";
import {
  MertonCalculator,
  type MertonCalculatorRequest,
  type MertonReportData,
} from "@/components/finstack/models/components/merton-calculator/merton-calculator";
import { unwrap } from "@/workers/finstack-contract";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./merton.json";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
type Resources = Record<
  | "mertonConstructed"
  | "mertonFreed"
  | "masterScaleConstructed"
  | "masterScaleFreed",
  number
>;
let harness: Awaited<ReturnType<typeof startWorker>>;
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
/** Merton and master-scale handles constructed and freed by one call. */
async function counted<T>(call: () => T) {
  const before = await resources();
  const value = await call();
  const after = await resources();
  return {
    value,
    merton: [
      after.mertonConstructed - before.mertonConstructed,
      after.mertonFreed - before.mertonFreed,
    ],
    scale: [
      after.masterScaleConstructed - before.masterScaleConstructed,
      after.masterScaleFreed - before.masterScaleFreed,
    ],
  };
}
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) => {
      if (method === "initialize")
        return unwrap(await harness.proxy.initialize());
      if (method === "mertonReport")
        return unwrap(
          await harness.proxy.mertonReport(request as MertonReportRequest),
        );
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const entry = (name: "model" | "equity") => ({
  request: fixture[name].request as MertonReportRequest,
  report: fixture[name].report as unknown as MertonReport,
});
const firm = JSON.parse(
  (entry("model").request.source as { json: string }).json,
);
const withModel = (model: object): MertonReportRequest => ({
  ...entry("model").request,
  source: { kind: "model", json: JSON.stringify(model) },
});
// Reports are regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/credit/generate.mjs).
it.each(["model", "equity"] as const)(
  "returns the %s-source native report through the real worker and frees both handles",
  async (name) => {
    const { request, report } = entry(name);
    const run = await counted(() => harness.proxy.mertonReport(request));
    expect(unwrap(run.value)).toEqual(report);
    expect(run.merton).toEqual([1, 1]);
    expect(run.scale).toEqual([1, 1]);
  },
  budget(15000),
);
it("grades the returned default probabilities and keeps the KMV default point of an equity source", () => {
  const model = entry("model").report;
  expect(model.defaultPoint).toBeNull();
  expect(model.grades.map((grade) => grade.input_pd)).toEqual(
    model.defaultProbabilities,
  );
  expect(model.horizons).toEqual(entry("model").request.horizons);
  const equity = entry("equity").report;
  // Short-term 30 plus half of long-term 40, as Rust returned it.
  expect(equity.defaultPoint).toBe(50);
  expect(equity.model.debt_barrier).toBe(50);
});
it("keeps the native error of a measure Rust rejects for the model beside the rest", async () => {
  // Rust defines the endogenous debt spread for a terminal barrier only.
  const firstPassage = await counted(() =>
    harness.proxy.mertonReport(
      withModel({
        ...firm,
        barrier_type: { first_passage: { barrier_growth_rate: 0 } },
      }),
    ),
  );
  const report = unwrap(firstPassage.value);
  expect(report.debtSpread.ok).toBe(false);
  if (!report.debtSpread.ok) {
    expect(report.debtSpread.error.kind).toBe("validation");
    expect(report.debtSpread.error.message).toContain("Terminal only");
  }
  expect(report.impliedSpread.ok).toBe(true);
  expect(report.cdsParSpread.ok).toBe(true);
  expect(firstPassage.merton).toEqual([1, 1]);
  expect(firstPassage.scale).toEqual([1, 1]);
  // A CDS recovery that contradicts the CreditGrades mean recovery.
  const creditGrades = unwrap(
    await harness.proxy.mertonReport(
      withModel({
        ...firm,
        barrier_type: { first_passage: { barrier_growth_rate: 0 } },
        dynamics: {
          credit_grades: { barrier_uncertainty: 0.3, mean_recovery: 0.5 },
        },
      }),
    ),
  );
  expect(creditGrades.cdsParSpread.ok).toBe(false);
  if (!creditGrades.cdsParSpread.ok)
    expect(creditGrades.cdsParSpread.error.message).toContain("mean_recovery");
});
it("returns the native error of a rejected model and of a rejected equity calibration", async () => {
  const bad = await counted(() =>
    harness.proxy.mertonReport(withModel({ ...firm, asset_vol: -0.25 })),
  );
  expect(bad.value.ok).toBe(false);
  expect(bad.merton).toEqual([0, 0]);
  expect(bad.scale).toEqual([0, 0]);
  const { request } = entry("equity");
  const kmv = await harness.proxy.mertonReport({
    ...request,
    source: { ...request.source, shortTermDebt: 0, longTermDebt: 0 } as never,
  });
  expect(kmv.ok).toBe(false);
  if (!kmv.ok) expect(kmv.error.kind).toBe("validation");
});
it("validates MertonModel JSON natively and frees the handle", async () => {
  const run = await counted(() =>
    harness.proxy.validateMertonModel(JSON.stringify(firm)),
  );
  expect(JSON.parse(unwrap(run.value))).toEqual(entry("model").report.model);
  expect(run.merton).toEqual([1, 1]);
  const failed = await harness.proxy.validateMertonModel(
    JSON.stringify({ ...firm, asset_vol: -0.25 }),
  );
  expect(failed.ok).toBe(false);
  if (!failed.ok) expect(failed.error.kind).toBe("validation");
});
it("keys the query on the worker session and the complete snapshotted request", () => {
  const { request } = entry("equity");
  const options = mertonReportOptions(null, 3, request);
  expect(options.queryKey).toEqual(["finstack", 3, "mertonReport", request]);
  expect(options.queryKey[3]).not.toBe(request);
  expect(mertonReportOptions(null, 3, null).enabled).toBe(false);
});
const calculatorRequest = (name: "model" | "equity") =>
  fixture[name].request as MertonCalculatorRequest;
const data = (name: "model" | "equity") =>
  fixture[name].report as unknown as MertonReportData;
it("shows the returned tiles, term structure, grades and model parameters", () => {
  const report = data("equity");
  render(
    <MertonCalculator
      defaultRequest={calculatorRequest("equity")}
      validate={async (json) => json}
      onSubmit={() => {}}
      report={report}
    />,
  );
  // Every returned digit by default; nothing rounded or scaled.
  const cds = screen.getByRole("region", {
    name: "CDS par spread (5y, recovery 0.4)",
  });
  if (!report.cdsParSpread.ok) throw new Error("fixture CDS spread");
  expect(within(cds).getByText(String(report.cdsParSpread.value))).toBeTruthy();
  const terms = within(
    screen.getByRole("region", { name: "Term structure" }),
  ).getByRole("table");
  expect(within(terms).getAllByRole("row")).toHaveLength(
    1 + report.horizons.length,
  );
  expect(
    within(terms).getByText(String(report.defaultProbabilities[0])),
  ).toBeTruthy();
  expect(within(terms).getAllByText(report.grades[0]!.grade).length).toBe(
    report.grades.filter((g) => g.grade === report.grades[0]!.grade).length,
  );
  const model = within(screen.getByRole("region", { name: "Model" })).getByRole(
    "table",
  );
  expect(within(model).getByText("KMV default point")).toBeTruthy();
  expect(
    within(model).getByText(String(report.model.asset_value)),
  ).toBeTruthy();
});
it("shows a measure's native error in place of its values", () => {
  const report = data("model");
  render(
    <MertonCalculator
      defaultRequest={calculatorRequest("model")}
      validate={async (json) => json}
      onSubmit={() => {}}
      report={{
        ...report,
        debtSpread: { ok: false, error: { message: "terminal only" } },
        impliedEquity: { ok: false, error: { message: "ill-conditioned" } },
      }}
    />,
  );
  expect(screen.getByText("Debt spread: terminal only")).toBeTruthy();
  expect(
    within(
      screen.getByRole("region", { name: "Implied equity value (5y)" }),
    ).getByText("ill-conditioned"),
  ).toBeTruthy();
});
it(
  "submits the equity calibration and the generated model form as typed",
  { timeout: budget(20000) },
  async () => {
    const onSubmit = vi.fn();
    render(
      <MertonCalculator
        defaultRequest={calculatorRequest("equity")}
        validate={async (json) => json}
        onSubmit={onSubmit}
        report={null}
      />,
    );
    expect(screen.getByRole("status").textContent).toBe("No Merton report");
    const recovery = screen.getByLabelText("Recovery");
    await userEvent.clear(recovery);
    await userEvent.type(recovery, "0.35");
    await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
    expect(onSubmit).toHaveBeenLastCalledWith({
      ...calculatorRequest("equity"),
      recovery: 0.35,
    });
    // A cleared field is sent as NaN for Rust to reject, never as 0.
    await userEvent.clear(screen.getByLabelText("Payout rate"));
    await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
    const cleared = onSubmit.mock.lastCall![0] as MertonCalculatorRequest;
    expect(
      cleared.source.kind === "equity" && cleared.source.payoutRate,
    ).toBeNaN();
    cleanup();
    const validate = vi.fn(async (json: string) => json);
    render(
      <MertonCalculator
        defaultRequest={calculatorRequest("model")}
        validate={validate}
        onSubmit={onSubmit}
        report={null}
      />,
    );
    // The generated form renders its MertonModel fields from the caller's JSON.
    await screen.findByText("Debt barrier", {}, { timeout: budget(5000) });
    await waitFor(() => expect(validate).toHaveBeenCalled(), {
      timeout: budget(5000),
    });
    await userEvent.click(screen.getByRole("button", { name: "Calculate" }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(3), {
      timeout: budget(5000),
    });
    const submitted = onSubmit.mock.calls[2]![0] as MertonCalculatorRequest;
    expect(submitted.horizons).toEqual(calculatorRequest("model").horizons);
    expect(submitted.source.kind).toBe("model");
    if (submitted.source.kind === "model")
      expect(JSON.parse(submitted.source.json)).toEqual(firm);
  },
);
function Connected({ request }: { request: MertonReportRequest }) {
  const report = useMertonReport(request);
  return (
    <MertonCalculator
      defaultRequest={request as MertonCalculatorRequest}
      validate={async (json) => json}
      onSubmit={() => {}}
      report={report.data ?? null}
      loading={report.isFetching}
      error={report.error?.message}
    />
  );
}
it("renders the worker report through the hook, then the native error", async () => {
  const { request } = entry("model");
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        screen.getByRole("region", { name: "Term structure" }),
      ).toBeTruthy(),
    { timeout: budget(5000) },
  );
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected request={withModel({ ...firm, asset_value: -1 })} />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
