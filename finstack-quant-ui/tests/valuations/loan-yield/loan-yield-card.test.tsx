// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
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
  metricValuesOptions,
  useMetricValues,
  type MetricValuesRequest,
} from "@/hooks/valuations/use-metric-values/use-metric-values";
import {
  applicableLoanYieldMetrics,
  LoanYieldCard,
  loanYieldMetrics,
  type LoanYieldRow,
} from "@/components/finstack/valuations/components/loan-yield-card/loan-yield-card";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { unwrap, type PriceRequest } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import type { MetricMetadata } from "finstack-quant-wasm";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
let harness: Awaited<ReturnType<typeof startWorker>>;
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
      if (method === "metricValues")
        return unwrap(
          await harness.proxy.metricValues(request as MetricValuesRequest),
        );
      throw new Error(`Unexpected worker call ${method}`);
    }),
    close: vi.fn(),
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const { cases } = fixture;
const rowsOf = (entry: { rows: unknown }) =>
  entry.rows as unknown as LoanYieldRow[];
const metadataOf = (entry: { metadata: unknown }) =>
  entry.metadata as unknown as MetricMetadata[];
const failing = (entry: { rows: unknown }) =>
  rowsOf(entry).flatMap((row) => (row.value.ok ? [] : [row.metric]));
const rowFor = (metric: string) =>
  screen.getByRole("rowheader", { name: labelFor(metric) }).closest("tr")!;

// Values are regenerated and compared by `mise run ui-fixtures --check`.
it("pins the JSON inputs the fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/valuations/loan-yield/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it.each(Object.entries(cases))(
  "the worker returns the native values and errors of %s",
  async (_name, entry) => {
    const rows = unwrap(await harness.proxy.metricValues(entry.request));
    expect(rows).toEqual(entry.rows);
  },
  budget(30000),
);
it("one failing id fails a whole native request, so the worker prices each on its own", async () => {
  const { instrumentJson, marketJson, asOf, model } = cases.fixed.request;
  const metrics = ["ytm", "discount_margin", "all_in_rate"];
  const request: PriceRequest = {
    instrumentJson,
    marketJson,
    asOf,
    model,
    metrics,
  };
  const whole = await harness.proxy.price(request);
  expect(whole.ok).toBe(false);
  if (whole.ok) return;
  expect(whole.error.message).toBe(fixture.wholeRequestError);
  const each = unwrap(await harness.proxy.metricValues(request as never));
  expect(each.map((row) => [row.metric, row.value.ok])).toEqual([
    ["ytm", true],
    ["discount_margin", false],
    ["all_in_rate", true],
  ]);
});
it("a malformed instrument is the error of the whole request", async () => {
  const result = await harness.proxy.metricValues({
    ...cases.fixed.request,
    instrumentJson: "{}",
  });
  expect(result.ok).toBe(false);
});
it("Rust lists every id of the card for a term loan and five of them for a revolver", () => {
  expect(cases.fixed.request.metrics).toEqual([...loanYieldMetrics]);
  expect(applicableLoanYieldMetrics(cases.fixed.available)).toEqual([
    ...loanYieldMetrics,
  ]);
  expect(applicableLoanYieldMetrics(cases.revolver.available)).toEqual([
    "ytm",
    "discount_margin",
    "all_in_rate",
    "oid_eir_rate",
    "oid_eir_amortization",
  ]);
});
it("shows every value of a quoted callable loan under the tree model as returned", () => {
  const entry = cases.quotedTree;
  expect(failing(entry)).toEqual([]);
  render(<LoanYieldCard rows={rowsOf(entry)} metadata={metadataOf(entry)} />);
  for (const row of rowsOf(entry)) {
    const cells = rowFor(row.metric).children;
    expect(cells[1].textContent).toBe(
      String((row.value as { value: number }).value),
    );
  }
  expect(screen.queryByRole("alert")).toBeNull();
});
it("labels by labelFor and states only the unit and group Rust declares", () => {
  const entry = cases.quotedTree;
  render(<LoanYieldCard rows={rowsOf(entry)} metadata={metadataOf(entry)} />);
  const cells = (metric: string) =>
    [...rowFor(metric).children].map((cell) => cell.textContent);
  expect(cells("yt3y")[0]).toBe("YT3Y");
  expect(cells("yt3y").slice(2)).toEqual([
    "not declared",
    "Instrument Specific",
  ]);
  expect(cells("ytm").slice(2)).toEqual(["decimal", "Pricing"]);
  expect(cells("embedded_option_value").slice(2)).toEqual([
    "currency",
    "Pricing",
  ]);
});
it.each([
  ["quotedDiscounting", "embedded_option_value", "require the Tree model"],
  ["modelTree", "discount_margin", "requires quoted_clean_price_pct"],
  ["fixed", "discount_margin", "only defined for floating-rate"],
] as const)(
  "%s keeps the error of %s beside the other rows",
  (name, metric, message) => {
    const entry = cases[name];
    expect(failing(entry)).toEqual([metric]);
    render(<LoanYieldCard rows={rowsOf(entry)} metadata={metadataOf(entry)} />);
    const alert = within(rowFor(metric)).getByRole("alert");
    expect(alert.textContent).toContain(message);
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    const ytm = rowsOf(entry)[0];
    expect(rowFor("ytm").children[1].textContent).toBe(
      String((ytm.value as { value: number }).value),
    );
  },
);
it("shows a value Rust returned as zero, and formats only for display", () => {
  const entry = cases.fixed;
  render(
    <LoanYieldCard
      rows={rowsOf(entry)}
      metadata={metadataOf(entry)}
      formatValue={(value) => value.toFixed(2)}
    />,
  );
  const option = rowFor("embedded_option_value").children[1];
  expect(option.textContent).toBe("0.00");
  expect(option.querySelector("[title]")!.getAttribute("title")).toBe("0");
});
it("shows the whole-request error, the loading state and the empty state", () => {
  const { rerender } = render(<LoanYieldCard rows={null} error="bad market" />);
  expect(screen.getByRole("alert").textContent).toBe("bad market");
  rerender(<LoanYieldCard rows={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Pricing metrics…");
  rerender(<LoanYieldCard rows={null} />);
  expect(screen.getByRole("status").textContent).toBe("No metric values");
  rerender(<LoanYieldCard rows={[]} />);
  expect(
    screen.getByText("Rust lists none of these metrics for this instrument"),
  ).toBeTruthy();
});
it("keys the values on the session and the complete request", () => {
  const client = { call: vi.fn(), close() {} } as unknown as FinstackClient;
  const request = cases.fixed.request;
  const options = metricValuesOptions(client, 3, request);
  expect(options.queryKey).toEqual(["finstack", 3, "metricValues", request]);
  expect(metricValuesOptions(client, 4, request).queryKey).not.toEqual(
    options.queryKey,
  );
  expect(
    metricValuesOptions(client, 3, { ...request, metrics: ["ytm"] }).queryKey,
  ).not.toEqual(options.queryKey);
  expect(metricValuesOptions(client, 3, null).enabled).toBe(false);
});
function Connected({ request }: { request: MetricValuesRequest }) {
  const query = useMetricValues(request);
  return (
    <LoanYieldCard
      rows={(query.data as LoanYieldRow[] | undefined) ?? null}
      loading={query.isFetching}
      error={query.error?.message}
    />
  );
}
it("reaches the worker through the hook", async () => {
  render(
    <FinstackQueryProvider>
      <Connected request={cases.fixed.request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () => expect(screen.getByRole("rowheader", { name: "YTM" })).toBeTruthy(),
    { timeout: budget(30000) },
  );
  expect(rowFor("discount_margin").textContent).toContain(
    "only defined for floating-rate",
  );
});
