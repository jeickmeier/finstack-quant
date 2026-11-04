// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  creditScoreOptions,
  useCreditScore,
  type CreditScore,
  type CreditScoreModel,
  type CreditScoreRequest,
} from "@/hooks/models/use-credit-score/use-credit-score";
import {
  CreditScorecard,
  type CreditScoreData,
  type CreditScorecardRequest,
} from "@/components/finstack/models/components/credit-scorecard/credit-scorecard";
import { scoringModels } from "@/components/finstack/models/components/credit-scorecard/scoring";
import { unwrap } from "@/workers/finstack-contract";
import { useWorkerHarness } from "../../shared/worker/use-worker-harness";
import { budget } from "../../shared/budget";
import fixture from "./scorecard.json";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const declarations = readFileSync(
  resolve(import.meta.dirname, "../../../../finstack-quant-wasm/index.d.ts"),
  "utf8",
);
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
type Resources = Record<"masterScaleConstructed" | "masterScaleFreed", number>;
const harness = useWorkerHarness(["creditScore"]);
const resources = () =>
  (harness.proxy as unknown as { resources(): Promise<Resources> }).resources();
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const models = scoringModels.map(({ model }) => model);
const entry = (model: CreditScoreModel) => ({
  request: fixture[model].request as CreditScoreRequest,
  score: fixture[model].score as unknown as CreditScore,
});
const camel = (field: string) =>
  field.replace(/_([a-z])/g, (_match, letter: string) => letter.toUpperCase());
/** The positional parameter names index.d.ts declares for a scoring function. */
function declared(model: CreditScoreModel) {
  const block = declarations.slice(
    declarations.indexOf("export interface CreditScoringNamespace"),
  );
  const signature = new RegExp(`\\n  ${model}\\(([^)]*)\\)`).exec(block)?.[1];
  if (!signature) throw new Error(`No declaration of ${model}`);
  return [...signature.matchAll(/(\w+): number/g)].map((match) => match[1]);
}
const contractOf = (model: CreditScoreModel) =>
  scoringModels.find((entry) => entry.model === model)!.contract;
// Scores are regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/credit/generate.mjs).
it.each(models)(
  "returns the %s fixture score through the real worker and frees the scale",
  async (model) => {
    const { request, score } = entry(model);
    const before = await resources();
    const value = unwrap(await harness.proxy.creditScore(request));
    const after = await resources();
    expect(JSON.parse(JSON.stringify(value))).toEqual(score);
    expect([
      after.masterScaleConstructed - before.masterScaleConstructed,
      after.masterScaleFreed - before.masterScaleFreed,
    ]).toEqual([1, 1]);
  },
  budget(15000),
);
it("returns the scores the Rust unit tests pin for their inputs", () => {
  // finstack-quant/models/src/credit/scoring/tests.rs
  const pinned: Partial<Record<CreditScoreModel, number>> = {
    altmanZScore: 3.595,
    altmanZPrime: 2.62415,
    altmanZDoublePrime: 3.366,
    altmanEmScore: 7.808,
    zmijewskiScore: -4.336 - 4.513 * 0.1 + 5.679 * 0.4 + 0.004 * 2,
  };
  for (const [model, score] of Object.entries(pinned))
    expect(entry(model as CreditScoreModel).score.result.score).toBeCloseTo(
      score,
      10,
    );
  // Altman results carry no implied PD, so Rust grades none of them.
  for (const model of models) {
    const { score } = entry(model);
    expect(score.grade.ok).toBe(score.result.implied_pd != null);
  }
});
it.each(models)(
  "pins the %s argument order: contract fields match the facade parameters and distinct ratios reach their positions",
  async (model) => {
    const fields = contractOf(model).required;
    expect(fields.map(camel)).toEqual(declared(model));
    // Distinct values, so a swapped position changes the score; the Ohlson
    // indicators must be exactly 0 or 1 and differ from each other.
    const input = Object.fromEntries(
      fields.map((field, index) => [
        field,
        field === "liabilities_exceed_assets"
          ? 1
          : field === "negative_net_income_two_years"
            ? 0
            : 0.11 * (index + 1),
      ]),
    );
    const value = unwrap(
      await harness.proxy.creditScore({ model, input } as CreditScoreRequest),
    );
    expect(value.result).toEqual(
      native[model](...fields.map((field) => input[field])),
    );
  },
  budget(15000),
);
it("passes missing and non-finite ratios through for Rust to reject", async () => {
  const { request } = entry("zmijewskiScore");
  const { net_income_to_total_assets: _omitted, ...missing } =
    request.input as Record<string, number>;
  const absent = await harness.proxy.creditScore({
    model: "zmijewskiScore",
    input: missing,
  } as unknown as CreditScoreRequest);
  expect(absent.ok).toBe(false);
  if (!absent.ok) expect(absent.error.kind).toBe("invalid_type");
  const nan = await harness.proxy.creditScore({
    model: "zmijewskiScore",
    input: { ...request.input, net_income_to_total_assets: Number.NaN },
  } as CreditScoreRequest);
  expect(nan.ok).toBe(false);
  if (!nan.ok) {
    expect(nan.error.kind).toBe("validation");
    expect(nan.error.message).toContain("net_income_to_total_assets");
  }
  const indicator = await harness.proxy.creditScore({
    model: "ohlsonOScore",
    input: {
      ...entry("ohlsonOScore").request.input,
      liabilities_exceed_assets: 0.5,
    },
  } as CreditScoreRequest);
  expect(indicator.ok).toBe(false);
  const unknown = await harness.proxy.creditScore({
    model: "altmanXScore",
    input: {},
  } as unknown as CreditScoreRequest);
  expect(unknown.ok).toBe(false);
  if (!unknown.ok)
    expect(unknown.error.message).toBe("Unsupported scoring model");
});
it("keys the query on the worker session and the complete request, keeping NaN distinct", () => {
  const { request } = entry("zmijewskiScore");
  const options = creditScoreOptions(null, 3, request);
  expect(options.queryKey.slice(0, 3)).toEqual(["finstack", 3, "creditScore"]);
  const nan = creditScoreOptions(null, 3, {
    ...request,
    input: { ...request.input, net_income_to_total_assets: Number.NaN },
  } as CreditScoreRequest);
  const infinite = creditScoreOptions(null, 3, {
    ...request,
    input: { ...request.input, net_income_to_total_assets: Infinity },
  } as CreditScoreRequest);
  expect(nan.queryKey[3]).not.toEqual(infinite.queryKey[3]);
  expect(creditScoreOptions(null, 3, null).enabled).toBe(false);
});
const data = (model: CreditScoreModel) =>
  fixture[model].score as unknown as CreditScoreData;
it("shows the returned score, zone, implied PD and grade, or Rust's grading error", () => {
  const zmijewski = data("zmijewskiScore");
  render(
    <CreditScorecard
      defaultModel="zmijewskiScore"
      onSubmit={() => {}}
      score={zmijewski}
    />,
  );
  const view = screen.getByRole("region", { name: "Credit score" });
  expect(within(view).getByText(zmijewski.result.model)).toBeTruthy();
  expect(
    within(within(view).getByRole("region", { name: "Score" })).getByText(
      String(zmijewski.result.score),
    ),
  ).toBeTruthy();
  expect(within(view).getByText(zmijewski.result.zone)).toBeTruthy();
  if (!zmijewski.grade.ok) throw new Error("fixture grade");
  expect(within(view).getByText(zmijewski.grade.value.grade)).toBeTruthy();
  cleanup();
  const altman = data("altmanZScore");
  render(
    <CreditScorecard
      defaultModel="altmanZScore"
      onSubmit={() => {}}
      score={altman}
    />,
  );
  if (altman.grade.ok) throw new Error("fixture Altman grade");
  expect(screen.getByText(altman.grade.error.message)).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "Implied pd" })).getByText("—"),
  ).toBeTruthy();
});
it(
  "renders each model's contract fields, keeps entries per model and submits them as typed",
  { timeout: budget(20000) },
  async () => {
    const onSubmit = vi.fn();
    const zmijewski = entry("zmijewskiScore").request;
    render(
      <CreditScorecard
        defaultModel="zmijewskiScore"
        defaultInputs={{ zmijewskiScore: zmijewski.input }}
        onSubmit={onSubmit}
        score={null}
      />,
    );
    expect(screen.getByRole("status").textContent).toBe("No score");
    // The fields of the generated ZmijewskiInput, in Rust order.
    const form = screen.getByRole("form", { name: "Scoring inputs" });
    expect(
      within(form)
        .getAllByRole("textbox")
        .map((input) => input.getAttribute("aria-label") ?? input.id),
    ).toHaveLength(3);
    await userEvent.click(screen.getByRole("button", { name: "Score" }));
    expect(onSubmit).toHaveBeenLastCalledWith(zmijewski);
    const leverage = screen.getByLabelText("Total liabilities to total assets");
    await userEvent.clear(leverage);
    await userEvent.type(leverage, "0.9");
    // A cleared field is sent as NaN for Rust to reject, never as 0.
    await userEvent.clear(screen.getByLabelText("Net income to total assets"));
    await userEvent.click(screen.getByRole("button", { name: "Score" }));
    const cleared = onSubmit.mock.lastCall![0] as CreditScorecardRequest;
    expect(cleared.model).toBe("zmijewskiScore");
    expect(cleared.input).toMatchObject({
      total_liabilities_to_total_assets: 0.9,
      current_assets_to_current_liabilities: 2,
    });
    expect(
      (cleared.input as Record<string, number>).net_income_to_total_assets,
    ).toBeNaN();
    // Another model starts empty and shows its own nine Ohlson fields.
    await userEvent.click(screen.getByRole("combobox", { name: "Model" }));
    await userEvent.click(
      await screen.findByRole("option", { name: "Ohlson O (1980)" }),
    );
    expect(within(form).getAllByRole("textbox")).toHaveLength(9);
    expect(screen.getByLabelText("Liabilities exceed assets")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Score" }));
    const ohlson = onSubmit.mock.lastCall![0] as CreditScorecardRequest;
    expect(ohlson.model).toBe("ohlsonOScore");
    expect(Object.keys(ohlson.input)).toEqual(
      contractOf("ohlsonOScore").required,
    );
    expect(Object.values(ohlson.input).every(Number.isNaN)).toBe(true);
  },
);
function Connected({ request }: { request: CreditScoreRequest }) {
  const score = useCreditScore(request);
  return (
    <CreditScorecard
      defaultModel={request.model}
      onSubmit={() => {}}
      score={score.data ?? null}
      loading={score.isFetching}
      error={score.error?.message}
    />
  );
}
it("renders the worker score through the hook, then the native error", async () => {
  const { request, score } = entry("ohlsonOScore");
  const { unmount } = render(
    <FinstackQueryProvider>
      <Connected request={request} />
    </FinstackQueryProvider>,
  );
  await waitFor(
    () => expect(screen.getByText(score.result.model)).toBeTruthy(),
    { timeout: budget(5000) },
  );
  unmount();
  render(
    <FinstackQueryProvider>
      <Connected
        request={
          {
            ...request,
            input: { ...request.input, net_income_change: Number.NaN },
          } as CreditScoreRequest
        }
      />
    </FinstackQueryProvider>,
  );
  await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy(), {
    timeout: budget(5000),
  });
});
