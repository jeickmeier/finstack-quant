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
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  computeQuotesOptions,
  useComputeQuotes,
  type ComputeQuotesRequest,
} from "@/hooks/valuations/use-compute-quotes/use-compute-quotes";
import {
  QuoteConverter,
  quoteSetMetrics,
  type QuoteConverterProps,
  type QuoteInput,
  type QuoteSet,
} from "@/components/finstack/valuations/components/quote-converter/quote-converter";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { unwrap } from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
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
      if (method === "computeQuotes")
        return unwrap(
          await harness.proxy.computeQuotes(request as ComputeQuotesRequest),
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
const { cases, errors } = fixture;
const metadata = fixture.metadata as QuoteConverterProps["metadata"];
const setOf = (entry: { quotes: unknown }) => entry.quotes as QuoteSet;
const inputOf = (entry: { request: { quoteInput: unknown } }) =>
  entry.request.quoteInput as QuoteInput;
const requestOf = (entry: { request: unknown }) =>
  entry.request as ComputeQuotesRequest;
const valueOf = (field: string) =>
  within(screen.getByRole("row", { name: new RegExp(`^${labelFor(field)}`) }));

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
      `${source.path} changed since tests/valuations/quote-converter/generate.mjs ran; rerun mise run ui-fixtures`,
    ).toBe(source.sha256);
});
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
      metadata={metadata}
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
  ).toBe(`${labelFor("clean_price_currency")} (currency)`);
  expect(
    within(rows[1]).getByRole("rowheader").textContent,
    "clean_price_pct has no metric, so no unit of its own",
  ).toBe(labelFor("clean_price_pct"));
});
it("draws a supplied value as it came, compared with nothing", () => {
  // ytw above ytm can't come from Rust's walk: nothing here clamps or reorders it.
  const swapped = { ...setOf(cases.clean), ytm: 0.01, ytw: 0.09 };
  render(
    <QuoteConverter
      defaultInput={inputOf(cases.clean)}
      onSubmit={() => {}}
      quotes={swapped}
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
      error={message}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(message);
  expect(message).toContain("z_spread");
  expect(screen.getByRole("button", { name: "Convert" })).toBeTruthy();
});
it("states loading and no result", () => {
  const props = { defaultInput: { ytm: 0.05 }, onSubmit: () => {} };
  const { rerender } = render(
    <QuoteConverter {...props} quotes={null} loading />,
  );
  expect(screen.getByRole("status").textContent).toBe("Solving quotes…");
  rerender(<QuoteConverter {...props} quotes={null} />);
  expect(screen.getByRole("status").textContent).toBe("No quote set");
});
it("asks metricMetadata for every id a field maps to", () => {
  expect(new Set(fixture.metadata.map((entry) => entry.key))).toEqual(
    new Set(quoteSetMetrics),
  );
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
  return (
    <QuoteConverter
      defaultInput={inputOf(cases.callable)}
      onSubmit={() => {}}
      quotes={(quotes.data as QuoteSet | undefined) ?? null}
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
  },
  budget(40000),
);
