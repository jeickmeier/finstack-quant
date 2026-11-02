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
import userEvent from "@testing-library/user-event";
import type { MetricMetadata } from "finstack-quant-wasm";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  useConvertibleSpotLadder,
  type ConvertibleSpotLadderRequest,
} from "@/hooks/valuations/use-convertible-spot-ladder/use-convertible-spot-ladder";
import {
  ConvertibleCard,
  convertibleMetrics,
  ladderMetrics,
  type ConvertibleCardProps,
} from "@/components/finstack/valuations/components/convertible-card/convertible-card";
import type { MetricValueRow } from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { unwrap } from "@/workers/finstack-contract";
import { startWorker } from "../../shared/worker/harness.mjs";
import { budget } from "../../shared/budget";
import fixture from "./cases.json";

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
      if (method === "convertibleSpotLadder")
        return unwrap(
          await harness.proxy.convertibleSpotLadder(
            request as ConvertibleSpotLadderRequest,
          ),
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
const props = (name: keyof typeof cases) => ({
  ladder: cases[name].ladder as ConvertibleCardProps["ladder"],
  rows: cases[name].rows as unknown as MetricValueRow[],
  metadata: fixture.metadata as unknown as MetricMetadata[],
});
const ladderRow = (text: string) =>
  within(screen.getByRole("region", { name: "Convertible by share price" }))
    .getByRole("rowheader", { name: new RegExp(`^${text}`) })
    .closest("tr")!;

it("shows each row and regime as Rust returned them, the market row marked", () => {
  render(<ConvertibleCard {...props("plain")} />);
  const { rows } = cases.plain.ladder;
  expect(rows.map((row) => row.spot)).toEqual([
    25, 50, 75, 100, 125, 150, 200, 300,
  ]);
  for (const row of rows) {
    const cells = ladderRow(String(row.spot)).querySelectorAll("td");
    expect(cells[1].textContent).toBe(String(row.delta));
    expect(cells[2].textContent).toBe(String(row.hedge_ratio));
    expect(cells[3].textContent).toBe(labelFor(row.regime as string));
    expect(cells[4].textContent).toBe(String(row.parity));
    expect(cells[5].textContent).toBe(String(row.conversion_premium));
  }
  // One row in each regime reaches the screen from Rust, not from a threshold here.
  expect(new Set(rows.map((row) => row.regime))).toEqual(
    new Set(["busted", "balanced", "equity_sensitive"]),
  );
  expect(ladderRow("100").querySelector("th")?.textContent).toBe(
    "100 (market)",
  );
  expect(
    screen.getByRole("img", { name: /Price, conversion value and bond floor/ }),
  ).toBeTruthy();
  expect(
    screen.getByRole("img", { name: /Parity and conversion premium/ }),
  ).toBeTruthy();
});
it("shows a mandatory-variable bond's null regime as undefined, never zero", () => {
  render(<ConvertibleCard {...props("mandatory")} />);
  const cells = ladderRow("100").querySelectorAll("td");
  expect(cases.mandatory.ladder.rows[3].regime).toBeNull();
  expect(cells[2].textContent).toBe("Undefined");
  expect(cells[3].textContent).toBe("Undefined");
});
it("shows an id Rust rejects as its native error, in its own row", () => {
  render(<ConvertibleCard {...props("plain")} />);
  const table = screen.getByRole("region", { name: "Convertible" });
  const oas = cases.plain.rows.find(({ metric }) => metric === "oas")!
    .value as {
    error: { message: string };
  };
  const row = within(table)
    .getByRole("rowheader", { name: labelFor("oas") })
    .closest("tr")!;
  expect(row.textContent).toContain(oas.error.message);
  const parity = within(table)
    .getByRole("rowheader", { name: labelFor("parity") })
    .closest("tr")!;
  expect(parity.children[1].textContent).toBe("1");
});
it("reads the solved rows of a quoted, callable convertible", () => {
  render(<ConvertibleCard {...props("quoted")} />);
  const table = screen.getByRole("region", { name: "Convertible" });
  for (const metric of ["oas", "implied_vol"]) {
    const native = cases.quoted.rows.find((row) => row.metric === metric)!
      .value as { value: number };
    const row = within(table)
      .getByRole("rowheader", { name: labelFor(metric) })
      .closest("tr")!;
    expect(row.children[1].textContent).toBe(String(native.value));
  }
});
it("hands the entered share prices on, and empty as Rust's ladder", async () => {
  const onSpotsChange = vi.fn();
  render(<ConvertibleCard {...props("plain")} onSpotsChange={onSpotsChange} />);
  const input = screen.getByRole("textbox", { name: /Share prices/ });
  await userEvent.type(input, "150, 50 100");
  await userEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(onSpotsChange).toHaveBeenLastCalledWith([150, 50, 100]);
  await userEvent.clear(input);
  await userEvent.click(screen.getByRole("button", { name: "Apply" }));
  expect(onSpotsChange).toHaveBeenLastCalledWith(null);
});
it("shows a ladder's native error as returned", () => {
  render(
    <ConvertibleCard
      ladder={null}
      ladderError="native ladder error"
      rows={null}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe("native ladder error");
});
function Live({ request }: { request: ConvertibleSpotLadderRequest }) {
  const ladder = useConvertibleSpotLadder(request);
  return (
    <ConvertibleCard
      ladder={(ladder.data as ConvertibleCardProps["ladder"]) ?? null}
      ladderLoading={ladder.isFetching}
      ladderError={ladder.error?.message}
      rows={null}
    />
  );
}
it("reaches Rust through the worker: the default ladder and supplied spots match native", async () => {
  for (const name of ["plain", "supplied"] as const) {
    const { request, ladder } = cases[name];
    const returned = unwrap(
      await harness.proxy.convertibleSpotLadder(
        request as ConvertibleSpotLadderRequest,
      ),
    );
    expect(returned).toEqual(ladder);
  }
  // Rust sorted and deduped [150, 50, 100, 150].
  expect(cases.supplied.ladder.rows.map((row) => row.spot)).toEqual([
    50, 100, 150,
  ]);
  render(
    <FinstackQueryProvider>
      <Live request={cases.plain.request as ConvertibleSpotLadderRequest} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () => expect(ladderRow("100").querySelector("th")).toBeTruthy(),
    { timeout: budget(15000) },
  );
  // A share price Rust rejects is the hook's error.
  const rejected = await harness.proxy.convertibleSpotLadder({
    ...(cases.plain.request as ConvertibleSpotLadderRequest),
    spots: [0],
  });
  expect(() => unwrap(rejected)).toThrow(/share price/);
});
it("holds the card's ids in the fixture generator, each listed by Rust", () => {
  const here = dirname(fileURLToPath(import.meta.url));
  const generator = readFileSync(resolve(here, "generate.mjs"), "utf8");
  for (const id of convertibleMetrics) expect(generator).toContain(`"${id}"`);
  const listed = Object.values(fixture.available).flat();
  for (const id of convertibleMetrics) expect(listed).toContain(id);
  for (const id of ladderMetrics) expect(listed).toContain(id);
  expect(fixture.metadata.map(({ key }) => key)).toEqual([
    ...convertibleMetrics,
  ]);
  // The fixture's sources have not moved since it was generated.
  for (const { path, sha256 } of fixture.sources) {
    const content = readFileSync(resolve(here, "../../../..", path));
    expect(createHash("sha256").update(content).digest("hex")).toBe(sha256);
  }
});
