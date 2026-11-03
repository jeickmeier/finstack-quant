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
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import {
  PortfolioReturns,
  type PortfolioReturnsInputs,
  type PortfolioReturnsProps,
} from "@/components/finstack/portfolio/components/portfolio-returns/portfolio-returns";
import {
  mwrXirrOptions,
  twrrLinkedOptions,
  twrrModifiedDietzOptions,
} from "@/hooks/portfolio/use-portfolio-returns/use-portfolio-returns";
import {
  PortfolioWorkbench,
  type PortfolioWorkbenchProps,
} from "@/components/finstack/portfolio/blocks/portfolio-workbench/portfolio-workbench";
import {
  unwrap,
  type Envelope,
  type WorkerApi,
} from "@/workers/finstack-contract";
import type { FinstackClient } from "@/hooks/shared/use-finstack/client";
import { startWorker } from "../shared/worker/harness.mjs";
import { budget } from "../shared/budget";
import fixture from "./returns/cases.json";
import workbench from "./workbench/cases.json";
import pricingMarket from "../valuations/instruments/pricing-market.json";

const mock = vi.hoisted(() => ({
  createClient: vi.fn(),
  calls: [] as string[],
}));
vi.mock("@/hooks/shared/use-finstack/client", () => ({
  createClient: mock.createClient,
}));
// Figures are drawn by the shared chart in the browser gates.
vi.mock(
  "@/components/finstack/shared/chart/finstack-chart/finstack-chart",
  async (importOriginal) => ({
    ...(await importOriginal<object>()),
    FinstackChart: ({ ariaLabel }: { ariaLabel: string }) => (
      <div role="img" aria-label={ariaLabel} />
    ),
  }),
);
let harness: Awaited<ReturnType<typeof startWorker>>;
const proxy = () => harness.proxy as unknown as WorkerApi;
beforeAll(async () => {
  harness = await startWorker();
}, budget(30000));
afterAll(async () => harness?.close());
beforeEach(() => {
  vi.stubGlobal("Worker", class {});
  mock.calls.length = 0;
  mock.createClient.mockReturnValue({
    call: vi.fn(async (method: string, request: unknown) => {
      mock.calls.push(method);
      return unwrap(await (proxy() as any)[method](request));
    }),
    close() {},
  });
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const inputs = fixture.inputs as PortfolioReturnsInputs;
const { cases, errors } = fixture;
type Form = "linked" | "dietz" | "xirr";
const call = (form: Form, request: unknown): Promise<Envelope<unknown>> =>
  form === "linked"
    ? proxy().twrrLinked(request as PortfolioReturnsInputs["linked"])
    : form === "dietz"
      ? proxy().twrrModifiedDietz(request as PortfolioReturnsInputs["dietz"])
      : proxy().mwrXirr(request as PortfolioReturnsInputs["xirr"]);
const successes = (["linked", "dietz", "xirr"] as const).flatMap((form) =>
  Object.entries(cases[form]).map(
    ([name, entry]) => [form, name, entry] as const,
  ),
);
const failures = (["linked", "dietz", "xirr"] as const).flatMap((form) =>
  Object.entries(errors[form]).map(
    ([name, entry]) => [form, name, entry] as const,
  ),
);
const idle = { value: null };
function view(overrides: Partial<PortfolioReturnsProps> = {}) {
  return render(
    <PortfolioReturns
      defaultInputs={inputs}
      onSubmitLinked={() => {}}
      onSubmitDietz={() => {}}
      onSubmitXirr={() => {}}
      linked={idle}
      dietz={idle}
      xirr={idle}
      {...overrides}
    />,
  );
}
const card = (name: string) => within(screen.getByRole("region", { name }));

it.each(successes)(
  "the worker returns the native %s result of %s",
  async (form, _name, entry) => {
    expect(unwrap(await call(form, entry.request))).toEqual(entry.result);
  },
  budget(30000),
);
it.each(failures)(
  "the worker returns the native %s error of %s",
  async (form, _name, entry) => {
    const result = await call(form, entry.request);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.message).toBe(entry.error.message);
    expect(result.error.kind).toBe(entry.error.kind);
  },
  budget(30000),
);
it(
  "the boundary rejects an empty return field, sent as NaN, rather than reading zero",
  async () => {
    const result = await proxy().twrrLinked({
      returns: [0.05, Number.NaN],
      horizonYears: 1,
    });
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.error.message).toBe(fixture.emptyField.message);
  },
  budget(30000),
);
it("Rust flags which linked return it annualised", () => {
  expect(cases.linked.annual.result.is_annualised).toBe(true);
  expect(cases.linked.quarter.result.is_annualised).toBe(false);
  expect(cases.linked.quarter.result.annualised).toBe(
    cases.linked.quarter.result.cumulative,
  );
});
it("starts each form from the host's data", () => {
  view();
  const linked = card("Linked time-weighted return");
  inputs.linked.returns.forEach((value, index) =>
    expect(
      (
        linked.getByRole("textbox", {
          name: new RegExp(`^Sub-period return ${index + 1}`),
        }) as HTMLInputElement
      ).value,
    ).toBe(String(value)),
  );
  expect(
    (linked.getByRole("textbox", { name: /^Horizon/ }) as HTMLInputElement)
      .value,
  ).toBe(String(inputs.linked.horizonYears));
  const dated = card("Money-weighted return (XIRR)");
  expect(
    dated.getAllByRole("button", { name: /^Remove cash flow/ }),
  ).toHaveLength(inputs.xirr.length);
});
it("submits the entered returns and horizon as twrrLinked takes them", () => {
  const onSubmitLinked = vi.fn();
  view({ onSubmitLinked });
  const linked = card("Linked time-weighted return");
  fireEvent.click(
    linked.getByRole("button", { name: "Remove sub-period return 3" }),
  );
  fireEvent.click(linked.getByRole("button", { name: "Add sub-period" }));
  fireEvent.change(
    linked.getByRole("textbox", { name: /^Sub-period return 3/ }),
    { target: { value: "0.04" } },
  );
  fireEvent.change(linked.getByRole("textbox", { name: /^Horizon/ }), {
    target: { value: "" },
  });
  fireEvent.click(linked.getByRole("button", { name: "Link returns" }));
  expect(onSubmitLinked).toHaveBeenCalledExactlyOnceWith({
    returns: [inputs.linked.returns[0], inputs.linked.returns[1], 0.04],
    horizonYears: Number.NaN,
  });
});
it("submits the period with the portfolio's signs and the flows' fractions", () => {
  const onSubmitDietz = vi.fn();
  view({ onSubmitDietz });
  const dietz = card("Modified-Dietz period return");
  fireEvent.click(dietz.getByRole("button", { name: "Add flow" }));
  fireEvent.change(dietz.getByRole("textbox", { name: /^Flow 2 amount/ }), {
    target: { value: "-200000" },
  });
  fireEvent.change(dietz.getByRole("textbox", { name: /^Flow 2 fraction/ }), {
    target: { value: "0.1" },
  });
  fireEvent.click(dietz.getByRole("button", { name: "Compute period return" }));
  expect(onSubmitDietz).toHaveBeenCalledExactlyOnceWith({
    ...inputs.dietz,
    cashflows: [
      ...inputs.dietz.cashflows!,
      { amount: -200000, fraction_of_period_remaining: 0.1 },
    ],
  });
});
it("submits the dated flows in the order entered, with the investor's signs", () => {
  const onSubmitXirr = vi.fn();
  view({ onSubmitXirr });
  const dated = card("Money-weighted return (XIRR)");
  fireEvent.click(dated.getByRole("button", { name: "Remove cash flow 2" }));
  fireEvent.click(dated.getByRole("button", { name: "Solve XIRR" }));
  expect(onSubmitXirr).toHaveBeenCalledExactlyOnceWith([
    inputs.xirr[0],
    inputs.xirr[2],
  ]);
});
it("shows every linked field as returned, and says when Rust did not annualise", () => {
  const annual = cases.linked.annual.result;
  view({ linked: { value: annual } });
  const result = within(screen.getByLabelText("Linked return"));
  expect(result.getByTitle(String(annual.cumulative))).toBeTruthy();
  expect(result.getByTitle(String(annual.annualised))).toBeTruthy();
  expect(result.getByText("Annualised (decimal per year)")).toBeTruthy();
  expect(result.getByText("Yes")).toBeTruthy();
  expect(result.getByText(String(annual.num_periods))).toBeTruthy();
  expect(screen.queryByRole("note")).toBeNull();
  cleanup();
  const quarter = cases.linked.quarter.result;
  view({ linked: { value: quarter } });
  expect(screen.getByText("Annualised (decimal)")).toBeTruthy();
  expect(screen.getByText("No")).toBeTruthy();
  expect(screen.getByRole("note").textContent).toContain("Not annualised");
});
it("shows the period and money-weighted returns as returned, with Rust's units", () => {
  const period = cases.dietz.contribution.result;
  const irr = cases.xirr.conventional.result;
  view({ dietz: { value: period }, xirr: { value: irr } });
  expect(
    card("Modified-Dietz period return").getByTitle(String(period)).textContent,
  ).toBe(String(period));
  expect(screen.getByLabelText("Modified-Dietz return").textContent).toContain(
    "decimal, not annualised",
  );
  expect(screen.getByLabelText("Money-weighted return").textContent).toContain(
    "decimal per year, Act/365F",
  );
  expect(
    card("Money-weighted return (XIRR)").getByTitle(String(irr)),
  ).toBeTruthy();
});
it("shows each form's native error in that form only, keeping the inputs", () => {
  const { message } = errors.xirr.twoSignChanges.error;
  view({ xirr: { value: null, error: message } });
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  expect(
    card("Money-weighted return (XIRR)").getByRole("alert").textContent,
  ).toBe(message);
  expect(screen.getByRole("button", { name: "Solve XIRR" })).toBeTruthy();
});
it("states loading and no result per form", () => {
  view({ linked: { value: null, loading: true } });
  expect(
    card("Linked time-weighted return").getByRole("status").textContent,
  ).toBe("Linking returns…");
  expect(
    card("Modified-Dietz period return").getByRole("status").textContent,
  ).toBe("No period return");
});
it("the hooks are disabled without a request and key on the whole request", () => {
  const client = {} as FinstackClient;
  expect(twrrLinkedOptions(client, 1, null).enabled).toBe(false);
  expect(twrrModifiedDietzOptions(client, 1, null).enabled).toBe(false);
  expect(mwrXirrOptions(client, 1, null).enabled).toBe(false);
  expect(twrrLinkedOptions(client, 1, inputs.linked).queryKey).toEqual([
    "finstack",
    1,
    "twrrLinked",
    inputs.linked,
  ]);
  expect(twrrModifiedDietzOptions(client, 1, inputs.dietz).queryKey).toEqual([
    "finstack",
    1,
    "twrrModifiedDietz",
    inputs.dietz,
  ]);
  expect(mwrXirrOptions(client, 1, inputs.xirr).queryKey).toEqual([
    "finstack",
    1,
    "mwrXirr",
    inputs.xirr,
  ]);
});
const props: PortfolioWorkbenchProps = {
  defaultSpecJson: workbench.specJson,
  marketJson: JSON.stringify(pricingMarket),
  asOf: workbench.request.asOf,
  returns: inputs,
};
function renderWorkbench(overrides: Partial<PortfolioWorkbenchProps> = {}) {
  return render(
    <FinstackQueryProvider>
      <PortfolioWorkbench {...props} {...overrides} />
    </FinstackQueryProvider>,
  );
}
it("offers the Returns tab only when the host supplies its data", () => {
  renderWorkbench({ returns: undefined });
  expect(screen.queryByRole("tab", { name: "Returns" })).toBeNull();
  cleanup();
  renderWorkbench();
  expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual([
    "Positions",
    "Risk",
    "Returns",
    "Margin",
    "Book",
  ]);
  expect(mock.calls).not.toContain("twrrLinked");
});
it(
  "computes the host's three returns natively when the tab opens, and a resubmitted form's native error",
  async () => {
    const user = userEvent.setup();
    renderWorkbench();
    await user.click(screen.getByRole("tab", { name: "Returns" }));
    const linked = cases.linked.annual.result;
    expect(
      await screen.findByTitle(
        String(linked.cumulative),
        {},
        { timeout: budget(10000) },
      ),
    ).toBeTruthy();
    expect(
      await screen.findByTitle(String(cases.dietz.contribution.result)),
    ).toBeTruthy();
    expect(
      await screen.findByTitle(String(cases.xirr.conventional.result)),
    ).toBeTruthy();
    expect(screen.queryAllByRole("alert")).toEqual([]);
    const dietz = card("Modified-Dietz period return");
    fireEvent.change(dietz.getByRole("textbox", { name: /^Flow 1 fraction/ }), {
      target: { value: "1.2" },
    });
    fireEvent.click(
      dietz.getByRole("button", { name: "Compute period return" }),
    );
    expect(
      (await screen.findByRole("alert", {}, { timeout: budget(10000) }))
        .textContent,
    ).toBe(errors.dietz.fraction.error.message);
    expect(
      mock.calls.filter((name) => name === "twrrModifiedDietz"),
    ).toHaveLength(2);
  },
  budget(30000),
);
