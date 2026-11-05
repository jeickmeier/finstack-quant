// @vitest-environment jsdom
import { createRequire } from "node:module";
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
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../../shared/worker/client"),
);
const harness = useWorkerHarness(["creditScore"]);
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const models = scoringModels.map(({ model }) => model);
const entry = (model: CreditScoreModel) => ({
  request: fixture[model].request as CreditScoreRequest,
  score: fixture[model].score as unknown as CreditScore,
});
const contractOf = (model: CreditScoreModel) =>
  scoringModels.find((entry) => entry.model === model)!.contract;
// Scores are regenerated and compared by `mise run ui-fixtures --check`
// (tests/models/credit/generate.mjs).
it.each(models)(
  "returns the %s fixture score through the real worker",
  async (model) => {
    const { request, score } = entry(model);
    const value = unwrap(await harness.proxy.creditScore(request));
    expect(JSON.parse(JSON.stringify(value))).toEqual(score);
  },
  budget(15000),
);
it("returns the scores the Rust unit tests pin for their inputs", () => {
  // finstack-quant/models/src/credit/scoring/tests.rs
  const pinned: Partial<Record<CreditScoreModel, number>> = {
    altman_z_score: 3.595,
    altman_z_prime: 2.62415,
    altman_z_double_prime: 3.366,
    altman_em_score: 7.808,
    zmijewski_score: -4.336 - 4.513 * 0.1 + 5.679 * 0.4 + 0.004 * 2,
  };
  for (const [model, score] of Object.entries(pinned))
    expect(entry(model as CreditScoreModel).score.result.score).toBeCloseTo(
      score,
      10,
    );
  // Altman results carry no implied PD, so Rust grades none of them.
  for (const model of models) {
    const { score } = entry(model);
    expect(score.grade != null).toBe(score.result.implied_pd != null);
  }
});
it.each(models)(
  "passes the %s request to the native creditScoreReport unchanged",
  async (model) => {
    // Distinct values, so a field reaching the wrong ratio changes the score;
    // the Ohlson indicators must be exactly 0 or 1.
    const input: object = Object.fromEntries(
      contractOf(model).required.map((field, index) => [
        field,
        field === "liabilities_exceed_assets"
          ? 1
          : field === "negative_net_income_two_years"
            ? 0
            : 0.11 * (index + 1),
      ]),
    );
    const request = { model, input } as CreditScoreRequest;
    const value = unwrap(await harness.proxy.creditScore(request));
    expect(value).toEqual(native.creditScoreReport(request));
  },
  budget(15000),
);
it("passes missing and non-finite ratios through for Rust to reject", async () => {
  const { request } = entry("zmijewski_score");
  if (request.model !== "zmijewski_score") throw new Error("fixture model");
  const { net_income_to_total_assets: _omitted, ...missing } = request.input;
  const absent = await harness.proxy.creditScore({
    model: "zmijewski_score",
    input: missing,
  } as unknown as CreditScoreRequest);
  expect(absent.ok).toBe(false);
  if (!absent.ok) {
    expect(absent.error.kind).toBe("validation");
    expect(absent.error.message).toContain("net_income_to_total_assets");
  }
  const nan = await harness.proxy.creditScore({
    model: "zmijewski_score",
    input: { ...request.input, net_income_to_total_assets: Number.NaN },
  } as CreditScoreRequest);
  expect(nan.ok).toBe(false);
  if (!nan.ok) expect(nan.error.kind).toBe("invalid_type");
  const indicator = await harness.proxy.creditScore({
    model: "ohlson_o_score",
    input: {
      ...entry("ohlson_o_score").request.input,
      liabilities_exceed_assets: 0.5,
    },
  } as CreditScoreRequest);
  expect(indicator.ok).toBe(false);
  if (!indicator.ok) expect(indicator.error.kind).toBe("validation");
  const unknown = await harness.proxy.creditScore({
    model: "altman_x_score",
    input: {},
  } as unknown as CreditScoreRequest);
  expect(unknown.ok).toBe(false);
  if (!unknown.ok) {
    expect(unknown.error.kind).toBe("validation");
    expect(unknown.error.message).toContain("altman_x_score");
  }
});
it("keys the query on the worker session and the complete request, keeping NaN distinct", () => {
  const { request } = entry("zmijewski_score");
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
it("shows the returned score, zone, implied PD and grade, or no grade without an implied PD", () => {
  const zmijewski = data("zmijewski_score");
  render(
    <CreditScorecard
      defaultModel="zmijewski_score"
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
  if (!zmijewski.grade) throw new Error("fixture grade");
  expect(within(view).getByText(zmijewski.grade.grade)).toBeTruthy();
  cleanup();
  const altman = data("altman_z_score");
  render(
    <CreditScorecard
      defaultModel="altman_z_score"
      onSubmit={() => {}}
      score={altman}
    />,
  );
  if (altman.grade) throw new Error("fixture Altman grade");
  expect(screen.getByText("No implied PD")).toBeTruthy();
  expect(
    within(screen.getByRole("region", { name: "Implied pd" })).getByText("—"),
  ).toBeTruthy();
});
it(
  "renders each model's contract fields, keeps entries per model and submits them as typed",
  { timeout: budget(20000) },
  async () => {
    const onSubmit = vi.fn();
    const zmijewski = entry("zmijewski_score").request;
    render(
      <CreditScorecard
        defaultModel="zmijewski_score"
        defaultInputs={{ zmijewski_score: zmijewski.input }}
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
    if (cleared.model !== "zmijewski_score") throw new Error(cleared.model);
    expect(cleared.input).toMatchObject({
      total_liabilities_to_total_assets: 0.9,
      current_assets_to_current_liabilities: 2,
    });
    expect(cleared.input.net_income_to_total_assets).toBeNaN();
    // Another model starts empty and shows its own nine Ohlson fields.
    await userEvent.click(screen.getByRole("combobox", { name: "Model" }));
    await userEvent.click(
      await screen.findByRole("option", { name: "Ohlson O (1980)" }),
    );
    expect(within(form).getAllByRole("textbox")).toHaveLength(9);
    expect(screen.getByLabelText("Liabilities exceed assets")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Score" }));
    const ohlson = onSubmit.mock.lastCall![0] as CreditScorecardRequest;
    expect(ohlson.model).toBe("ohlson_o_score");
    expect(Object.keys(ohlson.input)).toEqual(
      contractOf("ohlson_o_score").required,
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
  const { request, score } = entry("ohlson_o_score");
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
