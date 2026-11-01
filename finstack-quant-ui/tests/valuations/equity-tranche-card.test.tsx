// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import type {
  EquityMetricsFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import fixture from "./scenario-table/equity-tranche.json";
import { formatRaw } from "@/lib/finstack/format/format";
import { EquityTrancheCard } from "@/components/finstack/valuations/components/equity-tranche-card/equity-tranche-card";
import { EquityPanel } from "@/components/finstack/valuations/blocks/pricing-workbench/equity-panel";

const useEquityMetrics = vi.hoisted(() => vi.fn());
vi.mock("@/hooks/valuations/use-deal-report/use-deal-report", () => ({
  useEquityMetrics,
  useEquityMetricsFieldUnits: () => ({ isFetching: false, error: null }),
}));

type Metrics = ReturnType<StructuredCredit["equityMetrics"]>;
const caseOf = (name: string) => fixture.cases.find((c) => c.name === name)!;
const metricsOf = (name: string) =>
  caseOf(name).result.value as unknown as Metrics;
const units = fixture.units as EquityMetricsFieldUnits;
afterEach(cleanup);
const card = (metrics: Metrics) =>
  render(
    <EquityTrancheCard metrics={metrics} units={units} purchasePrice="" />,
  );
const tile = (label: string) =>
  screen.getByText(label, { selector: "dt" }).nextElementSibling!.textContent;
const rows = () =>
  within(screen.getByRole("table"))
    .getAllByRole("row")
    .map((row) =>
      [...row.querySelectorAll("th,td")].map((cell) => cell.textContent),
    );

it("captures the behaviours the card must keep apart", () => {
  const paying = metricsOf("paying");
  expect(paying.irr).toBeGreaterThan(0);
  expect(paying.moic).toBeGreaterThan(1);
  // The price moves the invested amount, the IRR and the MOIC, and not the NAV.
  const discounted = metricsOf("discounted");
  expect(discounted.irr).toBeGreaterThan(paying.irr!);
  expect(discounted.nav_pct).toBe(paying.nav_pct);
  // No IRR because nothing is received, with a true zero MOIC and NAV.
  const starved = metricsOf("starved");
  expect(starved.invested).toBeGreaterThan(0);
  expect([starved.irr, starved.moic, starved.nav_pct]).toEqual([null, 0, 0]);
  expect(starved.cash_on_cash).toEqual([]);
  // No IRR because nothing is invested: no MOIC or shares, with a NAV at a zero price.
  const free = metricsOf("zero-price");
  expect([free.invested, free.irr, free.moic]).toEqual([0, null, null]);
  expect(free.cash_on_cash.length).toBeGreaterThan(0);
  expect(free.cash_on_cash.every(([, share]) => share === null)).toBe(true);
  expect(free.nav_pct).toBe(paying.nav_pct);
  const written = metricsOf("zero-balance");
  expect([
    written.invested,
    written.irr,
    written.moic,
    written.nav_pct,
  ]).toEqual([0, null, null, 0]);
  // A class that receives little still has an IRR, near -100%.
  const wiped = metricsOf("wiped-out");
  expect(wiped.irr).toBeLessThan(-0.99);
  expect(wiped.moic).toBeGreaterThan(0);
  expect(caseOf("no-equity").result.error).toMatchObject({
    kind: "validation",
    message: expect.stringContaining("deal has no equity tranche"),
  });
  expect(caseOf("no-assets").result.error?.kind).toBe("not_found");
  expect(caseOf("negative-price").result.error?.kind).toBe("validation");
});

it("shows every returned value with every digit in Rust's units", () => {
  const metrics = metricsOf("paying");
  card(metrics);
  expect(
    screen.getByRole("region", { name: "Equity tranche" }).textContent,
  ).toContain(metrics.tranche_id);
  expect(tile("Invested")).toBe(`15,000,000${units.invested.label}`);
  expect(tile("IRR")).toBe(`${formatRaw(metrics.irr)}${units.irr.label}`);
  expect(tile("MOIC")).toBe(`${formatRaw(metrics.moic)}${units.moic.label}`);
  expect(tile("NAV pct")).toBe(
    `${formatRaw(metrics.nav_pct)}${units.nav_pct.label}`,
  );
  const [header, ...body] = rows();
  expect(header).toEqual([
    "Payment date",
    `Cash on cash (${units.cash_on_cash.label})`,
  ]);
  expect(body).toEqual(
    metrics.cash_on_cash.map(([date, share]) => [date, formatRaw(share)]),
  );
});

it("states a missing IRR and draws the zeros Rust returned as zero", () => {
  card(metricsOf("starved"));
  expect(tile("IRR")).toBe("No rate solves");
  expect(tile("MOIC")).toBe(`0${units.moic.label}`);
  expect(tile("NAV pct")).toBe(`0${units.nav_pct.label}`);
  expect(screen.getByRole("status").textContent).toBe(
    "Rust returned no distributions after the valuation date",
  );
});

it("states an undefined MOIC and shares when nothing is invested, without calling its NAV zero", () => {
  const metrics = metricsOf("zero-price");
  card(metrics);
  expect(tile("Invested")).toBe(`0${units.invested.label}`);
  expect(tile("IRR")).toBe("No rate solves");
  expect(tile("MOIC")).toBe("Nothing invested");
  expect(tile("NAV pct")).toBe(
    `${formatRaw(metrics.nav_pct)}${units.nav_pct.label}`,
  );
  expect(rows().slice(1)[0]![1]).toBe("Nothing invested");
});

it("draws a near-total loss's IRR rather than treating it as missing", () => {
  const metrics = metricsOf("wiped-out");
  card(metrics);
  expect(tile("IRR")).toBe(`${formatRaw(metrics.irr)}${units.irr.label}`);
});

it("shows Rust's error as returned and keeps the price editable", () => {
  const change = vi.fn();
  const message = caseOf("no-equity").result.error!.message;
  render(
    <EquityTrancheCard
      metrics={null}
      units={null}
      purchasePrice="60"
      onPurchasePriceChange={change}
      error={message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(message);
  expect(screen.queryByRole("table")).toBeNull();
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "70" },
  });
  expect(change).toHaveBeenCalledWith("70");
});

it("keeps the states of no result and no units", () => {
  render(
    <EquityTrancheCard metrics={null} units={null} purchasePrice="" loading />,
  );
  expect(screen.getByRole("status").textContent).toBe("Calculating…");
  cleanup();
  render(<EquityTrancheCard metrics={null} units={null} purchasePrice="" />);
  expect(screen.getByRole("status").textContent).toBe(
    "No equity metrics returned",
  );
  cleanup();
  render(
    <EquityTrancheCard
      metrics={metricsOf("paying")}
      units={null}
      purchasePrice=""
    />,
  );
  expect(screen.getByRole("status").textContent).toBe("No units returned");
});

it("omits a blank price, so Rust's default applies, and sends a typed one as typed", () => {
  useEquityMetrics.mockReturnValue({ isFetching: false, error: null });
  const request = {
    instrumentJson: "{}",
    marketJson: "{}",
    asOf: "2024-01-01",
  };
  render(<EquityPanel request={request} />);
  expect(useEquityMetrics).toHaveBeenLastCalledWith({
    ...request,
    purchasePricePct: null,
  });
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "60" } });
  expect(useEquityMetrics).toHaveBeenLastCalledWith({
    ...request,
    purchasePricePct: 60,
  });
});
