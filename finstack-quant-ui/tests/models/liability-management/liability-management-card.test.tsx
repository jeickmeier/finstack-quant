// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { useState } from "react";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import {
  exchangeOfferOptions,
  lmeOptions,
  useExchangeOffer,
  useLme,
  useTenderRecommendationHurdle,
} from "@/hooks/models/use-liability-management/use-liability-management";
import { ExchangeOfferCard } from "@/components/finstack/models/components/liability-management-card/exchange-offer-card";
import { LiabilityManagementCard } from "@/components/finstack/models/components/liability-management-card/liability-management-card";
import type { OutcomeState } from "@/components/finstack/shared/primitives/native-display/native-display";
import {
  unwrap,
  type Envelope,
  type ExchangeOfferRequest,
  type LmeRequest,
} from "@/workers/finstack-contract";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness([
  "analyzeExchangeOffer",
  "analyzeLme",
  "tenderRecommendationHurdle",
]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
type NativeOutcome =
  { ok: true; value: unknown } | { ok: false; error: { message: string } };
const expectOutcome = (actual: Envelope<unknown>, result: unknown) => {
  const expected = result as NativeOutcome;
  if (expected.ok) expect(unwrap(actual)).toEqual(expected.value);
  else {
    expect(actual.ok).toBe(false);
    if (!actual.ok) expect(actual.error.message).toBe(expected.error.message);
  }
};
const exchangeTerms = fixture.exchange.recommended
  .request as ExchangeOfferRequest;
const lmeTerms = fixture.lme.tender.request as LmeRequest;
const value = <T,>(entry: { result: unknown }) =>
  (entry.result as { ok: true; value: T }).value;
const offer = (entry: { result: unknown }) =>
  value<modelsWire.ExchangeOfferAnalysis>(entry);
const exercise = (entry: { result: unknown }) =>
  value<modelsWire.LmeAnalysis>(entry);
interface CardsProps {
  onExchangeSubmit?: (terms: ExchangeOfferRequest) => void;
  onLmeSubmit?: (terms: LmeRequest) => void;
  exchange?: OutcomeState<modelsWire.ExchangeOfferAnalysis>;
  lme?: OutcomeState<modelsWire.LmeAnalysis>;
  hurdle?: number | null;
}
/** The item's two cards, each fed its own analysis. */
function Cards({
  onExchangeSubmit = () => {},
  onLmeSubmit = () => {},
  exchange = { result: null },
  lme = { result: null },
  hurdle,
}: CardsProps) {
  return (
    <>
      <ExchangeOfferCard
        defaultTerms={exchangeTerms}
        onSubmit={onExchangeSubmit}
        state={exchange}
        hurdle={hurdle}
      />
      <LiabilityManagementCard
        defaultTerms={lmeTerms}
        onSubmit={onLmeSubmit}
        state={lme}
      />
    </>
  );
}
function card(props: CardsProps) {
  return render(<Cards {...props} />);
}
it("calls each native function with its arguments in the binding's positions", async () => {
  // Four distinct amounts: each comes back under its own field.
  const offered = unwrap(
    await harness.proxy.analyzeExchangeOffer({
      oldPv: 11,
      newPv: 22,
      consentFee: 3,
      equitySweetenerValue: 4,
      exchangeType: "uptier",
    }),
  );
  expect(offered).toMatchObject({
    exchange_type: "uptier",
    old_npv: 11,
    new_npv: 22,
    consent_fee: 3,
    equity_sweetener_value: 4,
  });
  // Cost is symmetric in price and participation, par retired is not.
  const retired = unwrap(
    await harness.proxy.analyzeLme({
      lmeType: "tender_offer",
      notional: 1000,
      repurchasePricePct: 0.5,
      optAcceptancePct: 0.25,
      ebitda: 200,
    }),
  );
  expect(retired.notional_reduction).toBe(250);
  expect(retired.cost).toBe(125);
  expect(retired.leverage_impact?.pre_leverage).toBe(5);
});
it.each(Object.entries(fixture.exchange))(
  "the worker returns the native exchange analysis of %s",
  async (_name, entry) => {
    expectOutcome(
      await harness.proxy.analyzeExchangeOffer(
        entry.request as ExchangeOfferRequest,
      ),
      entry.result,
    );
  },
);
it.each(Object.entries(fixture.lme))(
  "the worker returns the native LME analysis of %s",
  async (_name, entry) => {
    expectOutcome(
      await harness.proxy.analyzeLme(entry.request as LmeRequest),
      entry.result,
    );
  },
);
it("the worker returns the hurdle Rust applies", async () => {
  expect(unwrap(await harness.proxy.tenderRecommendationHurdle())).toBe(
    fixture.hurdle,
  );
});
it("takes the recommendation from Rust's field, whatever the hurdle prop says", () => {
  const recommended = offer(fixture.exchange.recommended);
  const { rerender } = card({
    exchange: { result: recommended },
    hurdle: 999,
  });
  expect(screen.getByText("Tender recommended")).toBeTruthy();
  expect(screen.getByText("999")).toBeTruthy();
  // The tender beats holding out here, yet Rust does not recommend it.
  const inside = offer(fixture.exchange.insideHurdle);
  expect(inside.delta_npv).toBeGreaterThan(0);
  rerender(<Cards exchange={{ result: inside }} hurdle={0} />);
  expect(screen.getByText("Tender not recommended")).toBeTruthy();
});
it("tables every other returned field of an offer as Rust returned it", () => {
  const result = offer(fixture.exchange.recommended);
  card({ exchange: { result } });
  const table = screen.getByRole("table", { name: "Offer economics" });
  expect(table.querySelectorAll("tbody tr")).toHaveLength(
    Object.keys(result).length - 1,
  );
  const row = within(table)
    .getByRole("rowheader", { name: "Delta NPV" })
    .closest("tr")!;
  expect(row.children[1].textContent).toBe(String(result.delta_npv));
  expect(
    within(table)
      .getByRole("rowheader", { name: "Exchange type" })
      .closest("tr")!.children[1].textContent,
  ).toBe(result.exchange_type);
});
it("adds the leverage block only when Rust returned one", () => {
  const { rerender } = card({
    lme: { result: exercise(fixture.lme.tender) },
  });
  const leverage = screen
    .getByRole("rowheader", { name: "Post leverage" })
    .closest("tr")!;
  expect(leverage.children[1].textContent).toBe(
    String(exercise(fixture.lme.tender).leverage_impact!.post_leverage),
  );
  rerender(<Cards lme={{ result: exercise(fixture.lme.amendAndExtend) }} />);
  expect(screen.queryByRole("rowheader", { name: "Post leverage" })).toBeNull();
  expect(screen.getByRole("rowheader", { name: "Cost" })).toBeTruthy();
});
it("submits typed numbers; an empty amount is NaN for Rust to reject and an empty EBITDA is unset", () => {
  const onExchangeSubmit = vi.fn();
  const onLmeSubmit = vi.fn();
  card({ onExchangeSubmit, onLmeSubmit });
  fireEvent.click(screen.getByRole("button", { name: "Analyze offer" }));
  expect(onExchangeSubmit).toHaveBeenLastCalledWith(exchangeTerms);
  fireEvent.change(screen.getByLabelText("Consent fee"), {
    target: { value: "" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Analyze offer" }));
  expect(onExchangeSubmit.mock.lastCall![0].consentFee).toBeNaN();
  fireEvent.click(screen.getByRole("button", { name: "Analyze LME" }));
  expect(onLmeSubmit).toHaveBeenLastCalledWith(lmeTerms);
  fireEvent.change(screen.getByLabelText("EBITDA"), { target: { value: "" } });
  fireEvent.click(screen.getByRole("button", { name: "Analyze LME" }));
  expect(onLmeSubmit.mock.lastCall![0].ebitda).toBeNull();
});
it("names the price input for what Rust reads it as under each structure", () => {
  card({});
  expect(screen.getByLabelText("Tender price")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "Dropdown" }));
  expect(screen.getByLabelText("Assets transferred")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "Amend and extend" }));
  expect(screen.getByLabelText("Extension fee")).toBeTruthy();
});
it("shows a native error and keeps the other analysis", () => {
  const entry = fixture.lme.pricePoints;
  expect(entry.result.ok).toBe(false);
  card({
    exchange: { result: offer(fixture.exchange.recommended) },
    lme: { result: null, error: entry.result.error!.message },
  });
  expect(screen.getByRole("alert").textContent).toContain(
    "repurchase_price_pct must be in",
  );
  expect(screen.getByText("Tender recommended")).toBeTruthy();
});
it("shows the loading and empty states", () => {
  card({ exchange: { result: null, loading: true } });
  expect(screen.getByText("Analyzing…")).toBeTruthy();
  expect(screen.getByText("No LME analysis")).toBeTruthy();
});
it("keys each analysis on the session and the complete terms", () => {
  const client = { call: vi.fn(), close() {} } as unknown as FinstackClient;
  const options = exchangeOfferOptions(client, 3, exchangeTerms);
  expect(options.queryKey).toEqual([
    "finstack",
    3,
    "analyzeExchangeOffer",
    exchangeTerms,
  ]);
  expect(exchangeOfferOptions(client, 4, exchangeTerms).queryKey).not.toEqual(
    options.queryKey,
  );
  expect(exchangeOfferOptions(client, 3, null).enabled).toBe(false);
  expect(
    lmeOptions(client, 3, { ...lmeTerms, notional: 1 }).queryKey,
  ).not.toEqual(lmeOptions(client, 3, lmeTerms).queryKey);
  expect(lmeOptions(client, 3, null).enabled).toBe(false);
});
function Connected() {
  const [offerTerms, setOfferTerms] = useState(exchangeTerms);
  const [lme, setLme] = useState(lmeTerms);
  const exchangeQuery = useExchangeOffer(offerTerms);
  const lmeQuery = useLme(lme);
  const hurdle = useTenderRecommendationHurdle();
  return (
    <Cards
      onExchangeSubmit={setOfferTerms}
      onLmeSubmit={setLme}
      exchange={{
        result: exchangeQuery.data,
        loading: exchangeQuery.isFetching,
        error: exchangeQuery.error?.message,
      }}
      lme={{
        result: lmeQuery.data,
        loading: lmeQuery.isFetching,
        error: lmeQuery.error?.message,
      }}
      hurdle={hurdle.data}
    />
  );
}
it("runs both analyses and the native rejection of an empty amount through the hooks", async () => {
  render(
    <FinstackQueryProvider>
      <Connected />
    </FinstackQueryProvider>,
  );
  await screen.findByText("Tender recommended");
  await screen.findByRole("rowheader", { name: "Post leverage" });
  await screen.findByText(String(fixture.hurdle));
  fireEvent.change(screen.getByLabelText("Consent fee"), {
    target: { value: "" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Analyze offer" }));
  await waitFor(
    () =>
      expect(screen.getByRole("alert").textContent).toContain("consent_fee"),
    { timeout: budget(10000) },
  );
});
