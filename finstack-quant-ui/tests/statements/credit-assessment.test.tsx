// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterAll, afterEach, beforeAll, expect, it, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { QueryClient, hashKey } from "@tanstack/react-query";
import assessment from "../../src/fixtures/statements/analyst-credit-assessment.json";
import result from "../../src/fixtures/statements/analyst-result.json";
import plSummary from "../../src/fixtures/statements/analyst-pl-summary.json";
import { CreditAssessmentReport } from "@/components/finstack/statements/components/credit-assessment-report/credit-assessment-report";
import { creditAssessmentOptions } from "@/hooks/statements/use-credit-assessment/use-credit-assessment";
import { plSummaryReportOptions } from "@/hooks/statements/use-pl-summary-report/use-pl-summary-report";
import type { CreditAssessmentWire } from "@/lib/finstack/generated/types/credit_assessment";
import type { TableEnvelopeWire } from "@/lib/finstack/generated/types/table_envelope";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  unwrap,
  type Envelope,
  type WorkerApi,
} from "@/workers/finstack-contract";
import { serializeHost } from "../../src/codec.mjs";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";

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
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const resultsJson = JSON.stringify(result);
const captured = assessment as unknown as CreditAssessmentWire;
const envelope = plSummary as unknown as TableEnvelopeWire;
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
});
afterAll(async () => harness?.close());
afterEach(cleanup);

/** Client over the real worker that records every operation and argument it forwards. */
function recordingClient() {
  const calls: [keyof WorkerApi, unknown][] = [];
  const client = {
    call: async (method: keyof WorkerApi, argument: unknown) => {
      calls.push([method, argument]);
      const invoke = harness.proxy[method] as unknown as (
        argument: unknown,
      ) => Promise<Envelope<unknown>>;
      return unwrap(await invoke(argument));
    },
  } as unknown as FinstackClient;
  return { client, calls };
}

it(
  "assesses credit and builds the P&L summary through the real worker exactly as the facade returns them",
  async () => {
    const { client, calls } = recordingClient();
    const credit = { resultsJson, period: "2025Q4" };
    const pl = {
      resultsJson,
      lineItems: ["revenue", "ebitda", "net_income"],
      periods: ["2025Q3", "2025Q4"],
    };
    const creditOptions = creditAssessmentOptions(client, 1, credit);
    const plOptions = plSummaryReportOptions(client, 1, pl);
    credit.period = "changed after snapshot";
    pl.lineItems.reverse();
    const query = new QueryClient();
    try {
      const returned = await query.fetchQuery(creditOptions);
      expect(serializeHost(returned)).toBe(
        serializeHost(native.creditAssessment(resultsJson, "2025Q4")),
      );
      // The captured gallery fixture is this same native assessment.
      expect(JSON.parse(serializeHost(returned))).toEqual(captured);
      expect(serializeHost(await query.fetchQuery(plOptions))).toBe(
        serializeHost(
          native.plSummaryReport(
            resultsJson,
            ["revenue", "ebitda", "net_income"],
            ["2025Q3", "2025Q4"],
          ),
        ),
      );
      expect(calls).toEqual([
        ["creditAssessment", { resultsJson, period: "2025Q4" }],
        [
          "plSummaryReport",
          {
            resultsJson,
            lineItems: ["revenue", "ebitda", "net_income"],
            periods: ["2025Q3", "2025Q4"],
          },
        ],
      ]);
      // A malformed period is rejected natively and returned unchanged.
      await expect(
        query.fetchQuery(
          creditAssessmentOptions(client, 1, { resultsJson, period: "Q9" }),
        ),
      ).rejects.toMatchObject({ kind: "validation" });
    } finally {
      query.clear();
    }
  },
  budget(30000),
);

it("keys both queries by every request field and the worker session, and disables a null request", () => {
  const { client } = recordingClient();
  const expectKeyed = <R extends object>(
    options: (
      client: FinstackClient,
      session: number,
      request: R | null,
    ) => { queryKey: readonly unknown[]; enabled?: unknown },
    request: R,
  ) => {
    const key = hashKey(options(client, 1, request).queryKey);
    expect(hashKey(options(client, 2, request).queryKey)).not.toBe(key);
    for (const field of Object.keys(request))
      expect(
        hashKey(
          options(client, 1, { ...request, [field]: "changed" }).queryKey,
        ),
      ).not.toBe(key);
    expect(options(client, 1, request).enabled).toBe(true);
    expect(options(client, 1, null).enabled).toBe(false);
  };
  expectKeyed(creditAssessmentOptions, { resultsJson, period: "2025Q4" });
  expectKeyed(plSummaryReportOptions, {
    resultsJson,
    lineItems: ["revenue"],
    periods: ["2025Q4"],
  });
});

it("presents the returned assessment, lists the periods Rust left without a value and shows the P&L summary", () => {
  render(
    <CreditAssessmentReport
      assessment={captured}
      plSummary={envelope}
      currency="USD"
    />,
  );
  const report = screen.getByRole("region", { name: "Credit assessment" });
  expect(
    within(report).getByRole("heading", { name: /Credit assessment at/ })
      .textContent,
  ).toBe("Credit assessment at 2025Q4");
  const tile = (name: string) =>
    within(report).getByRole("region", { name }).querySelector("p")!
      .textContent;
  expect(tile("Leverage ratio")).toBe("1.9188563760404616");
  expect(tile("Interest coverage")).toBe("6.194873653258923");
  expect(tile("Free cash flow (USD)")).toBe("39,354,332.67060436");
  expect(
    within(report).getByRole("img", {
      name: "Leverage ratio and interest coverage by period",
    }),
  ).toBeTruthy();
  expect(
    within(report).getByText(
      "Not plotted, returned without a value: Leverage ratio 2024Q1, 2024Q2, 2024Q3; Interest coverage 2024Q1, 2024Q2, 2024Q3.",
    ),
  ).toBeTruthy();
  const table = within(report).getByRole("table", {
    name: "Credit assessment by period",
  });
  const rows = within(table).getAllByRole("row");
  // Header plus one row per returned period, in Rust's order.
  expect(rows).toHaveLength(captured.series.length + 1);
  expect(
    within(rows[1]!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual(["2024Q1", "—", "—", "25,419,187.5"]);
  expect(
    within(rows.at(-1)!)
      .getAllByRole("cell")
      .map((cell) => cell.textContent),
  ).toEqual([
    "2025Q4",
    "1.9188563760404616",
    "6.194873653258923",
    "39,354,332.67060436",
  ]);
  const pl = within(report).getByRole("region", { name: "P&L summary" });
  expect(within(pl).getAllByRole("row").length).toBe(envelope.row_count + 1);
});

it("shows native errors as returned and an injected format without computing", () => {
  const { rerender } = render(
    <CreditAssessmentReport
      assessment={null}
      plSummary={null}
      error="Validation error: bad period"
      plSummaryError="Validation error: bad line"
    />,
  );
  expect(screen.getAllByRole("alert").map((node) => node.textContent)).toEqual([
    "Validation error: bad period",
    "Validation error: bad line",
  ]);
  rerender(
    <CreditAssessmentReport
      assessment={captured}
      formatValue={(measure, value) => `${measure}=${value}`}
    />,
  );
  expect(
    screen.getByRole("region", { name: "Free cash flow" }).querySelector("p")!
      .textContent,
  ).toBe("free_cash_flow=39354332.67060436");
  expect(screen.queryByRole("region", { name: "P&L summary" })).toBeNull();
});
