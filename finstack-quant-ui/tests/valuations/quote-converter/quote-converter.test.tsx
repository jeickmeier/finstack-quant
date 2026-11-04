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
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  computeQuotesOptions,
  useBondQuoteSetFieldUnits,
  useComputeQuotes,
  type ComputeQuotesRequest,
} from "@/hooks/valuations/use-compute-quotes/use-compute-quotes";
import {
  QuoteConverter,
  type QuoteConverterProps,
  type QuoteInput,
  type QuoteSet,
} from "@/components/finstack/valuations/components/quote-converter/quote-converter";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { unwrap } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";

vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness(["bondQuoteSetFieldUnits", "computeQuotes"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const { cases, errors, currency } = fixture;
const units = fixture.units as QuoteConverterProps["units"];
const setOf = (entry: { quotes: unknown }) => entry.quotes as QuoteSet;
const inputOf = (entry: { request: { quoteInput: unknown } }) =>
  entry.request.quoteInput as QuoteInput;
const requestOf = (entry: { request: unknown }) =>
  entry.request as ComputeQuotesRequest;
const valueOf = (field: string) =>
  within(screen.getByRole("row", { name: new RegExp(`^${labelFor(field)}`) }));

// Values are regenerated and compared by `mise run ui-fixtures --check`.
it.each(Object.entries(cases))(
  "the worker returns the native set of %s",
  async (_name, entry) => {
    expect(unwrap(await harness.proxy.computeQuotes(requestOf(entry)))).toEqual(
      entry.quotes,
    );
  },
  budget(30000),
);
it.each(Object.entries(cases))(
  "each quote of %s is what priceInstrument measures at that price",
  (_name, entry) => {
    const quotes = setOf(entry) as unknown as Record<string, number>;
    for (const [metric, measured] of Object.entries(entry.metrics))
      expect(quotes[metric], metric).toBe(measured);
  },
);
it.each(Object.entries(errors))(
  "the worker returns the native error of %s",
  async (_name, entry) => {
    const result = await harness.proxy.computeQuotes(requestOf(entry));
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.message).toBe(entry.error.message);
    expect(result.error.kind).toBe(entry.error.kind);
  },
  budget(30000),
);
it("offers the kinds the BondQuoteInput schema accepts, in its order", async () => {
  render(
    <QuoteConverter
      defaultInput={{ clean_price_pct: 99 }}
      onSubmit={() => {}}
      quotes={null}
      units={null}
    />,
  );
  fireEvent.click(screen.getByRole("combobox"));
  const options = await screen.findAllByRole("option");
  expect(options.map((option) => option.textContent)).toEqual(
    contracts.bond_quote_input.schema.oneOf.map((branch) =>
      labelFor(branch.required[0]),
    ),
  );
});
it("submits the entered kind and value as Rust's single-key quote", () => {
  const onSubmit = vi.fn();
  render(
    <QuoteConverter
      defaultInput={{ ytm: 0.05 }}
      onSubmit={onSubmit}
      quotes={null}
      units={null}
    />,
  );
  fireEvent.change(screen.getByRole("textbox"), {
    target: { value: "0.0625" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Convert" }));
  expect(onSubmit).toHaveBeenCalledExactlyOnceWith({ ytm: 0.0625 });
});
it(
  "submits an empty value as NaN, and Rust rejects it rather than reading zero",
  async () => {
    const onSubmit = vi.fn();
    render(
      <QuoteConverter
        defaultInput={{ ytm: 0.05 }}
        onSubmit={onSubmit}
        quotes={null}
        units={null}
      />,
    );
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "" } });
    fireEvent.click(screen.getByRole("button", { name: "Convert" }));
    expect(onSubmit).toHaveBeenCalledExactlyOnceWith({ ytm: Number.NaN });
    const result = await harness.proxy.computeQuotes({
      ...requestOf(cases.clean),
      quoteInput: { ytm: Number.NaN },
    });
    expect(result.ok).toBe(false);
  },
  budget(30000),
);
it("draws every field Rust returned, in its order, with its declared unit", () => {
  const quotes = setOf(cases.clean);
  render(
    <QuoteConverter
      defaultInput={inputOf(cases.clean)}
      onSubmit={() => {}}
      quotes={quotes}
      units={units}
    />,
  );
  const rows = screen.getAllByRole("row").slice(1);
  expect(rows).toHaveLength(Object.keys(quotes).length);
  Object.entries(quotes).forEach(([field, value], index) => {
    expect(rows[index].textContent).toContain(labelFor(field));
    if (value !== null)
      expect(within(rows[index]).getByTitle(String(value))).toBeTruthy();
  });
  expect(valueOf("ytm").getByRole("rowheader").textContent).toBe(
    "YTM (decimal)",
  );
  expect(
    valueOf("clean_price_currency").getByRole("rowheader").textContent,
  ).toBe(`${labelFor("clean_price_currency")} (${currency})`);
  expect(within(rows[1]).getByRole("rowheader").textContent).toBe(
    `${labelFor("clean_price_pct")} (% of par)`,
  );
});
it("lists a unit for every field of the set, in the set's order", () => {
  expect(fixture.units.map((unit) => unit.field)).toEqual(
    Object.keys(setOf(cases.clean)),
  );
});
it(
  "the worker returns the native units",
  async () => {
    expect(
      unwrap(await harness.proxy.bondQuoteSetFieldUnits(currency)),
    ).toEqual(fixture.units);
  },
  budget(30000),
);
it("draws a supplied value as it came, compared with nothing", () => {
  // ytw above ytm can't come from Rust's walk: nothing here clamps or reorders it.
  const swapped = { ...setOf(cases.clean), ytm: 0.01, ytw: 0.09 };
  render(
    <QuoteConverter
      defaultInput={inputOf(cases.clean)}
      onSubmit={() => {}}
      quotes={swapped}
      units={null}
    />,
  );
  expect(valueOf("ytm").getByTitle("0.01")).toBeTruthy();
  expect(valueOf("ytw").getByTitle("0.09")).toBeTruthy();
});
it("shows a null field as no value, never zero", () => {
  render(
    <QuoteConverter
      defaultInput={inputOf(cases.clean)}
      onSubmit={() => {}}
      quotes={setOf(cases.clean)}
      units={null}
    />,
  );
  expect(setOf(cases.clean).discount_margin).toBeNull();
  expect(valueOf("discount_margin").getByText("No value")).toBeTruthy();
  cleanup();
  render(
    <QuoteConverter
      defaultInput={inputOf(cases.frn)}
      onSubmit={() => {}}
      quotes={setOf(cases.frn)}
      units={null}
    />,
  );
  expect(valueOf("asw_market").getByText("No value")).toBeTruthy();
  expect(
    valueOf("discount_margin").getByTitle(
      String(setOf(cases.frn).discount_margin),
    ),
  ).toBeTruthy();
});
it("shows the native error of a failed solve as returned, and keeps the form", () => {
  const { message } = errors.quoteDateCall.error;
  render(
    <QuoteConverter
      defaultInput={{ clean_price_pct: 100 }}
      onSubmit={() => {}}
      quotes={null}
      units={null}
      error={message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(message);
  expect(message).toContain("z_spread");
  expect(screen.getByRole("button", { name: "Convert" })).toBeTruthy();
});
it("states loading and no result", () => {
  const props = {
    defaultInput: { ytm: 0.05 },
    onSubmit: () => {},
    units: null,
  };
  const { rerender } = render(
    <QuoteConverter {...props} quotes={null} loading />,
  );
  expect(screen.getByRole("status").textContent).toBe("Solving quotes…");
  rerender(<QuoteConverter {...props} quotes={null} />);
  expect(screen.getByRole("status").textContent).toBe("No quote set");
});
it("the hook is disabled without a request and keys on the whole request", () => {
  const client = {} as FinstackClient;
  expect(computeQuotesOptions(client, 1, null).enabled).toBe(false);
  const request = requestOf(cases.clean);
  const options = computeQuotesOptions(client, 1, request);
  expect(options.enabled).toBe(true);
  expect(options.queryKey).toEqual(["finstack", 1, "computeQuotes", request]);
});
function Live() {
  const quotes = useComputeQuotes(requestOf(cases.callable));
  const fieldUnits = useBondQuoteSetFieldUnits(currency);
  return (
    <QuoteConverter
      defaultInput={inputOf(cases.callable)}
      onSubmit={() => {}}
      quotes={(quotes.data as QuoteSet | undefined) ?? null}
      units={fieldUnits.data ?? null}
      error={quotes.error?.message}
    />
  );
}
it(
  "the hook reaches the worker and draws the native result",
  async () => {
    render(
      <FinstackQueryProvider>
        <Live />
      </FinstackQueryProvider>,
    );
    await waitFor(() => expect(valueOf("ytw")).toBeTruthy(), {
      timeout: budget(30000),
    });
    expect(
      valueOf("ytw").getByTitle(String(setOf(cases.callable).ytw)),
    ).toBeTruthy();
    await waitFor(
      () =>
        expect(valueOf("ytw").getByRole("rowheader").textContent).toBe(
          "YTW (decimal)",
        ),
      { timeout: budget(30000) },
    );
  },
  budget(40000),
);
