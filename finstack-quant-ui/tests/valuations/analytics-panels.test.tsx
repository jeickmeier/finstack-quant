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
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { unwrap, type PriceRequest } from "@/workers/finstack-contract";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import bondCredit from "./bond-credit/cases.json";
import loanYield from "./loan-yield/cases.json";
import scenarioCases from "./scenario-table/cases.json";
import pricingMarket from "./instruments/pricing-market.json";
const mock = vi.hoisted(() => ({ createClient: vi.fn() }));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<
      typeof import("@/components/finstack/shared/chart/finstack-chart/finstack-chart")
    >()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
let harness: Awaited<ReturnType<typeof startWorker>>;
const called = vi.fn();
let rejecting: string | null = null;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  called.mockClear();
  rejecting = null;
  vi.stubGlobal("Worker", class {});
  mock.createClient.mockReturnValue({
    call: async (method: string, ...args: unknown[]) => {
      called(method, ...args);
      if (method === rejecting) throw new Error("native ladder error");
      return unwrap(await (harness.proxy as any)[method](...args));
    },
    close: vi.fn(),
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const openAnalytics = async (request: PriceRequest) => {
  render(
    <FinstackQueryProvider>
      <PricingWorkbench defaultRequest={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () =>
      expect(
        document.querySelector(".finstack-workbench__state")?.textContent,
      ).toBe("Priced"),
    { timeout: budget(15000) },
  );
  await userEvent.click(screen.getByRole("tab", { name: "Analytics" }));
};
const methods = () => called.mock.calls.map(([method]) => method);
it(
  "mounts the bond cards on their own, each with its own native error",
  async () => {
    const { request, rows } = bondCredit.cases.callable;
    await openAnalytics({ ...request, metrics: [] });
    // Nothing is priced until a card opens.
    expect(methods()).not.toContain("metricValues");
    await userEvent.click(screen.getByText("Spreads, risk and carry"));
    const credit = await screen.findByRole(
      "region",
      { name: "Bond credit" },
      { timeout: budget(15000) },
    );
    await within(credit).findByRole(
      "rowheader",
      { name: labelFor("oas") },
      { timeout: budget(15000) },
    );
    const row = (metric: string) =>
      within(credit)
        .getByRole("rowheader", { name: labelFor(metric) })
        .closest("tr")!;
    // The request's own model and overrides reach the worker.
    const native = rows.find(({ metric }) => metric === "oas")!.value as {
      value: number;
    };
    expect(row("oas").children[1].textContent).toBe(String(native.value));
    expect(within(row("g_spread")).getByRole("alert").textContent).toContain(
      "requires a government curve",
    );
    expect(methods()).not.toContain("bondWorkoutLadder");
    await userEvent.click(screen.getByText("Yield workout ladder"));
    await screen.findByRole(
      "region",
      { name: "Yield workout ladder" },
      { timeout: budget(15000) },
    );
    expect(methods()).toContain("bondWorkoutLadder");
    await userEvent.click(screen.getByText("Quote converter"));
    await screen.findByRole("region", { name: "Quote converter" });
    // The converter prices nothing until a quote is submitted.
    expect(methods()).not.toContain("computeQuotes");
  },
  budget(45000),
);
it(
  "a ladder that fails shows its error and leaves the credit card's values",
  async () => {
    const { request, rows } = bondCredit.cases.bullet;
    rejecting = "bondWorkoutLadder";
    await openAnalytics({ ...request, metrics: [] });
    await userEvent.click(screen.getByText("Yield workout ladder"));
    const ladder = await screen.findByRole("region", {
      name: "Yield workout ladder",
    });
    await waitFor(
      () =>
        expect(within(ladder).getByRole("alert").textContent).toContain(
          "native ladder error",
        ),
      { timeout: budget(10000) },
    );
    await userEvent.click(screen.getByText("Spreads, risk and carry"));
    const credit = await screen.findByRole(
      "region",
      { name: "Bond credit" },
      { timeout: budget(15000) },
    );
    await within(credit).findByRole(
      "rowheader",
      { name: labelFor("z_spread") },
      { timeout: budget(15000) },
    );
    expect(within(credit).getAllByRole("alert")).toHaveLength(
      rows.filter(({ value }) => !value.ok).length,
    );
    expect(within(ladder).getByRole("alert")).toBeTruthy();
  },
  budget(45000),
);
it(
  "mounts the loan card for a term loan, with Rust's error in a row",
  async () => {
    const { request } = loanYield.cases.quotedDiscounting;
    await openAnalytics({ ...request, metrics: [] });
    const card = await screen.findByRole(
      "region",
      { name: "Loan yields" },
      { timeout: budget(15000) },
    );
    await within(card).findByRole(
      "rowheader",
      { name: labelFor("ytm") },
      { timeout: budget(15000) },
    );
    expect(
      within(
        within(card)
          .getByRole("rowheader", { name: labelFor("embedded_option_value") })
          .closest("tr")!,
      ).getByRole("alert").textContent,
    ).toContain("require the Tree model");
  },
  budget(45000),
);
it(
  "offers no Analytics tab for an instrument without a panel",
  async () => {
    render(
      <FinstackQueryProvider>
        <PricingWorkbench
          defaultRequest={{
            instrumentJson: JSON.stringify(scenarioCases.cases[0]!.instrument),
            marketJson: JSON.stringify(pricingMarket),
            asOf: scenarioCases.asOf,
            model: "discounting",
            metrics: [],
          }}
        />
      </FinstackQueryProvider>,
    );
    await screen.findByRole(
      "tab",
      { name: "Scenarios" },
      { timeout: budget(15000) },
    );
    expect(screen.queryByRole("tab", { name: "Analytics" })).toBeNull();
  },
  budget(30000),
);
