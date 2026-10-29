// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
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
import type { MoneyValue, PortfolioMarginResult } from "finstack-quant-wasm";
import { portfolioMarginOptions } from "@/hooks/portfolio/use-portfolio-margin/use-portfolio-margin";
import { PortfolioMarginReport } from "@/components/finstack/portfolio/components/portfolio-margin-report/portfolio-margin-report";
import {
  unwrap,
  type PortfolioMarginRequest,
} from "@/workers/finstack-contract";
import { createContext } from "@/workers/services/context";
import { createPortfolioService } from "@/workers/services/portfolio";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./margin/cases.json";
import pricingMarket from "../valuations/instruments/pricing-market.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
let harness: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
afterEach(cleanup);
const entry = (name: string) =>
  fixture.cases.find((candidate) => candidate.name === name)!;
const result = (name: string) =>
  entry(name).result as unknown as PortfolioMarginResult;
const request = (name: string): PortfolioMarginRequest => ({
  specJson: fixture.specJson,
  marketJson: JSON.stringify(pricingMarket),
  asOf: fixture.asOf,
  currentImCollateral: entry(name).currentImCollateral as Record<
    string,
    MoneyValue
  > | null,
});
const rowsOf = (table: HTMLElement) =>
  (within(table).getAllByRole("row") as HTMLTableRowElement[]).slice(1);
// Margin runs are regenerated and compared by `mise run ui-fixtures --check`
// (tests/portfolio/margin/generate.mjs).
it("pins the JSON inputs the margin fixture was built from", () => {
  const repository = resolve(
    dirname(fileURLToPath(import.meta.url)),
    "../../..",
  );
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(repository, source.path)))
        .digest("hex"),
      `${source.path} changed since tests/portfolio/margin/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
it.each(["no-collateral", "usd-collateral-held"])(
  "runs the %s portfolio margin through the real worker exactly as the native fixture",
  async (name) => {
    const returned = unwrap(await harness.proxy.portfolioMargin(request(name)));
    // The fixture stores JSON, where Rust's -0 variation margin stays "-0".
    expect(JSON.parse(JSON.stringify(returned))).toEqual(entry(name).result);
  },
  budget(15000),
);
it("pins Rust's margin figures: two netting sets, a counted deposit and a -0 variation margin", () => {
  const run = result("usd-collateral-held");
  expect(run.netting_sets.map((set) => set.csa_id)).toEqual([
    "CSA-B",
    "USD-REGULATORY-CSA",
  ]);
  expect(run.netting_sets.map((set) => set.initial_margin.amount)).toEqual([
    "564110.756722781",
    "282055.3783613904",
  ]);
  expect(run.netting_sets.every((set) => set.is_approximate)).toBe(true);
  expect(run.netting_sets[0]!.variation_margin.amount).toBe("-0");
  expect(run.total_margin.amount).toBe("846166.135084171");
  expect(run.total_positions).toBe(2);
  expect(run.positions_without_margin).toBe(1);
  expect(run.by_csa["USD-REGULATORY-CSA"]!.transfer.amount).toBe("-1000000");
  expect(
    result("no-collateral").by_csa["USD-REGULATORY-CSA"]!.transfer,
  ).toEqual({ amount: "0", currency: "USD" });
});
it("returns the native validation error for collateral under an unknown CSA", async () => {
  const failed = await harness.proxy.portfolioMargin(request("unknown-csa"));
  expect(failed.ok).toBe(false);
  if (!failed.ok) {
    expect(failed.error.kind).toBe("validation");
    expect(failed.error.message).toBe(entry("unknown-csa").message);
  }
});
it("leaves omitted collateral unset and passes supplied collateral unchanged", async () => {
  const portfolioMargin = vi.fn(native.portfolioMargin);
  const service = createPortfolioService(
    { portfolio: { ...native, portfolioMargin } },
    createContext({
      initialize: async () => undefined,
      core: { MarketContext: native.MarketContext },
      portfolio: { Portfolio: native.Portfolio },
    }),
  );
  unwrap(await service.portfolioMargin(request("no-collateral")));
  unwrap(await service.portfolioMargin(request("usd-collateral-held")));
  expect(portfolioMargin.mock.calls.map((call) => call.slice(2))).toEqual([
    [fixture.asOf, undefined],
    [fixture.asOf, entry("usd-collateral-held").currentImCollateral],
  ]);
});
it("keys the margin run on the worker session and the complete snapshotted request", () => {
  const supplied = request("usd-collateral-held");
  const options = portfolioMarginOptions(null, 5, supplied);
  expect(options.queryKey).toEqual([
    "finstack",
    5,
    "portfolioMargin",
    supplied,
  ]);
  expect(options.queryKey[3]).not.toBe(supplied);
  expect(portfolioMarginOptions(null, 5, null).enabled).toBe(false);
});
it("shows the returned totals, counts, netting sets and CSA collateral as returned", () => {
  const run = result("usd-collateral-held");
  render(<PortfolioMarginReport result={run} density="comfortable" />);
  const report = screen.getByRole("region", { name: "Portfolio margin" });
  expect(within(report).getByText("Indicative")).toBeTruthy();
  const totals = within(report).getByLabelText("Totals");
  expect(
    within(totals).getByText("Positions without margin").nextSibling!
      .textContent,
  ).toBe("1");
  expect(
    within(totals).getByText("Positions in netting sets").nextSibling!
      .textContent,
  ).toBe("2");
  expect(within(totals).getByTitle(run.total_im_transfer.amount)).toBeTruthy();
  const sets = rowsOf(
    within(report).getByRole("table", { name: /netting set/ }),
  );
  // Returned order; the -0 variation margin is shown as returned.
  expect(sets.map((row) => row.cells[0]!.textContent)).toEqual([
    "CSA-Bbilateral · CSA CSA-B",
    "USD-REGULATORY-CSAbilateral · CSA USD-REGULATORY-CSA",
  ]);
  expect(
    sets[0]!.cells[4]!.querySelector("[title]")!.getAttribute("title"),
  ).toBe("-0");
  expect(sets[0]!.cells[1]!.textContent).toBe("simmApproximate");
  const csas = rowsOf(
    within(report).getByRole("table", { name: /collateral by CSA/ }),
  );
  expect(csas.map((row) => row.cells[0]!.textContent)).toEqual([
    "CSA-B",
    "USD-REGULATORY-CSA",
  ]);
  expect(within(report).getByText("No position was degraded.")).toBeTruthy();
});
it("shows the selected netting set's IM breakdown and SIMM sensitivities", () => {
  const run = result("no-collateral");
  render(<PortfolioMarginReport result={run} />);
  const first = screen.getByRole("region", {
    name: "Netting set CSA-B · CSA CSA-B",
  });
  const breakdown = rowsOf(
    within(first).getByRole("table", { name: /by component/ }),
  );
  expect(breakdown.map((row) => row.cells[0]!.textContent)).toEqual([
    "IR_Delta",
  ]);
  expect(
    within(first).getByRole("region", { name: "SIMM sensitivities" }),
  ).toBeTruthy();
  const sets = rowsOf(screen.getByRole("table", { name: /netting set/ }));
  fireEvent.click(sets[1]!.cells[0]!);
  expect(
    screen.getByRole("region", {
      name: "Netting set USD-REGULATORY-CSA · CSA USD-REGULATORY-CSA",
    }),
  ).toBeTruthy();
});
it("shows the empty, loading and native-error states", () => {
  const { rerender } = render(<PortfolioMarginReport result={null} />);
  expect(screen.getByRole("status").textContent).toBe("No portfolio margin");
  rerender(<PortfolioMarginReport result={null} loading />);
  expect(screen.getByRole("status").textContent).toBe(
    "Calculating portfolio margin…",
  );
  rerender(
    <PortfolioMarginReport
      result={result("no-collateral")}
      error={entry("unknown-csa").message!}
    />,
  );
  expect(screen.getByRole("alert").textContent).toContain(
    "Unknown IM collateral CSA",
  );
});
