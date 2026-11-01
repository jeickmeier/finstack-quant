// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import type { StructuredCredit } from "finstack-quant-wasm";
import fixture from "./scenario-table/waterfall-report.json";
import { groupDecimal } from "@/lib/finstack/format/format";
import { WaterfallReport } from "@/components/finstack/valuations/components/waterfall-report/waterfall-report";

type Run = ReturnType<StructuredCredit["runSimulation"]>;
const runOf = (name: string) =>
  fixture.cases.find((c) => c.name === name)!.run as unknown as Run;
const periodsOf = (run: Run) => run.diagnostics.periods!;
const digits = (money: { amount: string }) => groupDecimal(money.amount);
afterEach(cleanup);
const cells = (row: Element) =>
  [...row.querySelectorAll("th,td")].map((cell) => cell.textContent);
const bodyRows = (caption: string) =>
  within(screen.getByRole("table", { name: caption }))
    .getAllByRole("row")
    .slice(1)
    .map(cells);

it("shows one row per returned period in Rust's order", () => {
  const run = runOf("diverting");
  render(<WaterfallReport run={run} />);
  const body = screen.getByRole("table", { name: "By payment period (USD)" });
  expect(within(body).getAllByRole("row").slice(1).map(cells)).toEqual(
    periodsOf(run).map(
      ({ payment_date, waterfall, non_waterfall_payments }) => [
        `▸${payment_date}`,
        digits(waterfall.total_available),
        digits(waterfall.recovery_proceeds),
        digits(waterfall.diverted_cash),
        digits(waterfall.remaining_interest),
        digits(waterfall.remaining_principal),
        waterfall.had_diversions ? "Yes" : "No",
        String(non_waterfall_payments.length),
      ],
    ),
  );
});

it("expands a period to its tiers, records and diversions as returned", () => {
  const run = runOf("diverting");
  const [first] = periodsOf(run);
  expect(first!.waterfall.diverted_amounts.length).toBeGreaterThan(0);
  render(<WaterfallReport run={run} />);
  expect(screen.queryByRole("table", { name: "Payment records" })).toBeNull();
  const toggle = screen.getByRole("button", {
    name: new RegExp(first!.payment_date),
  });
  fireEvent.click(toggle);
  expect(toggle.getAttribute("aria-expanded")).toBe("true");
  expect(bodyRows("Tier allocations")).toEqual(
    first!.waterfall.tier_allocations.map(([tier, amount]) => [
      tier,
      digits(amount),
    ]),
  );
  expect(bodyRows("Payment records")).toEqual(
    first!.waterfall.payment_records.map((record) => [
      record.tier_id,
      record.recipient_id,
      String(record.priority),
      digits(record.requested_amount),
      digits(record.paid_amount),
      digits(record.shortfall),
      record.is_principal ? "Yes" : "No",
      record.diverted ? "Yes" : "No",
    ]),
  );
  expect(bodyRows("Diversions")).toEqual(
    first!.waterfall.diverted_amounts.map((diversion) => [
      diversion.source_tier,
      diversion.target_tranche,
      digits(diversion.amount),
      diversion.reason,
    ]),
  );
  expect(screen.getByText("None this period")).toBeTruthy();
  expect(
    screen.getByText(/^Principal used for interest:/).textContent,
  ).toContain(digits(first!.waterfall.principal_used_for_interest));
  fireEvent.click(toggle);
  expect(screen.queryByRole("table", { name: "Payment records" })).toBeNull();
});

it("lists a later recovery under its own date on the last period", () => {
  const run = runOf("diverting");
  const last = periodsOf(run).at(-1)!;
  expect(last.non_waterfall_payments.length).toBeGreaterThan(0);
  render(<WaterfallReport run={run} />);
  fireEvent.click(
    screen.getByRole("button", { name: new RegExp(last.payment_date) }),
  );
  expect(bodyRows("Non-waterfall payments").map((row) => row[0])).toEqual(
    last.non_waterfall_payments.map((payment) => payment.payment_date),
  );
});

it("renders a settlement-only deal as its one period, not as empty", () => {
  const run = runOf("settlement");
  const periods = periodsOf(run);
  expect(periods).toHaveLength(1);
  const [settlement] = periods;
  expect(settlement!.non_waterfall_payments.length).toBeGreaterThan(0);
  render(<WaterfallReport run={run} />);
  fireEvent.click(
    screen.getByRole("button", { name: new RegExp(settlement!.payment_date) }),
  );
  expect(bodyRows("Non-waterfall payments")).toEqual(
    settlement!.non_waterfall_payments.map((payment) => [
      payment.payment_date,
      payment.tranche_id,
      expect.any(String),
      digits(payment.interest),
      digits(payment.principal),
      digits(payment.incentive_fee),
    ]),
  );
  expect(screen.getAllByText("None this period")).toHaveLength(3);
});

it("says so when a run holds no period at all", () => {
  const run = runOf("settlement");
  render(
    <WaterfallReport
      run={{ ...run, diagnostics: { ...run.diagnostics, periods: [] } }}
    />,
  );
  expect(screen.getByRole("status").textContent).toBe(
    "The simulation returned no periods",
  );
});

it("shows pending and native error states", () => {
  const { rerender } = render(<WaterfallReport run={null} loading />);
  expect(screen.getByRole("status").textContent).toBe("Calculating…");
  rerender(<WaterfallReport run={null} error="native failure" />);
  expect(screen.getByRole("alert").textContent).toBe("native failure");
});
