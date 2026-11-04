// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FinstackQueryProvider } from "@/hooks/shared/use-finstack/use-finstack";
import { MarketScenarioBuilder } from "@/components/finstack/scenarios/components/market-scenario-builder/market-scenario-builder";
import { scenarioSpecModule } from "@/components/finstack/scenarios/components/market-scenario-builder/scenario-spec";
import { discriminator } from "@/components/finstack/core/components/schema-form/discriminator";
import { useScenarioSpecValidator } from "@/hooks/scenarios/use-scenario-spec-validator/use-scenario-spec-validator";
import {
  listBuiltinTemplateMetadataOptions,
  useListBuiltinTemplateMetadata,
} from "@/hooks/scenarios/use-list-builtin-template-metadata/use-list-builtin-template-metadata";
import { useBuildFromTemplate } from "@/hooks/scenarios/use-build-from-template/use-build-from-template";
import { useBuildTemplateComponent } from "@/hooks/scenarios/use-build-template-component/use-build-template-component";
import { useComposeScenarios } from "@/hooks/scenarios/use-compose-scenarios/use-compose-scenarios";
import { useBuildMarketScenario } from "@/hooks/scenarios/use-market-scenario/use-market-scenario";
import { unwrap } from "@/workers/finstack-contract";
import { useWorkerHarness } from "../shared/worker/use-worker-harness";
import { budget } from "../shared/budget";
import cases from "./templates/cases.json";
import bond from "../../src/fixtures/results/bond.json";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
vi.mock(
  "@/hooks/shared/use-finstack/client",
  () => import("../shared/worker/client"),
);
// Every call goes to the production worker service.
const harness = useWorkerHarness("any");
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const canonical = (spec: unknown) =>
  JSON.stringify(native.parseScenarioSpec(JSON.stringify(spec)));
const calls = (method: string) =>
  harness.call.mock.calls.filter(([name]) => name === method);
// Applications are regenerated and compared by `mise run ui-fixtures --check`
// (tests/scenarios/templates/generate.mjs).
it("returns the native templates, components and composition through the real worker", async () => {
  expect(unwrap(await harness.proxy.listBuiltinTemplateMetadata())).toEqual(
    cases.metadata,
  );
  expect(
    unwrap(await harness.proxy.buildFromTemplate(cases.templateId)),
  ).toEqual(cases.template);
  expect(
    unwrap(
      await harness.proxy.buildTemplateComponent({
        templateId: cases.templateId,
        componentId: cases.componentIds.rates,
      }),
    ),
  ).toEqual(cases.rates);
  expect(
    unwrap(
      await harness.proxy.composeScenarios([
        JSON.stringify(cases.remapped),
        JSON.stringify(cases.equity),
      ]),
    ),
  ).toEqual(cases.composed);
  expect(
    unwrap(
      await harness.proxy.validateScenarioSpec(JSON.stringify(cases.composed)),
    ),
  ).toBe(canonical(cases.composed));
  const unknown = await harness.proxy.buildFromTemplate("no_such_template");
  expect(unknown.ok).toBe(false);
});
it(
  "reports the template's failures and warnings as Rust returns them",
  async () => {
    const [asBuilt, whole, remapped] = cases.applications;
    // A placeholder curve id missing from the market fails the application.
    expect(asBuilt!.error).toEqual({
      name: "FinstackError",
      kind: "not_found",
      message: "Market data not found: USD-SOFR",
    });
    expect(whole!.error?.kind).toBe("not_found");
    const failed = await harness.proxy.applyMarketScenario({
      scenarioJson: asBuilt!.scenarioJson,
      marketJson: bond.request.marketJson,
      asOf: bond.request.asOf,
      instrumentJson: bond.request.instrumentJson,
    });
    expect(failed.ok).toBe(false);
    if (!failed.ok) expect(failed.error.message).toBe(asBuilt!.error!.message);
    // Remapped to USD-OIS the curve shift applies; missing equities only warn.
    const applied = unwrap(
      await harness.proxy.applyMarketScenario({
        scenarioJson: remapped!.scenarioJson,
        marketJson: bond.request.marketJson,
        asOf: bond.request.asOf,
        instrumentJson: bond.request.instrumentJson,
      }),
    );
    expect(applied.report).toEqual(remapped!.report);
    expect(applied.report.warnings).toEqual([
      { kind: "equity_not_found", id: "SPX" },
      { kind: "equity_not_found", id: "NDX" },
    ]);
  },
  budget(15000),
);
it("discriminates every OperationSpec variant on its kind tag", () => {
  const union = discriminator(scenarioSpecModule.schema as never, {
    schema: scenarioSpecModule.schema.properties.operations.items as never,
    pointer: "#/properties/operations/items",
  });
  expect(union).not.toBeNull();
  const kinds = union!.branches.map((branch) => branch.label);
  expect(kinds).toHaveLength(24);
  expect(new Set(kinds).size).toBe(24);
  expect(union!.branches.every((branch) => branch.tag === "kind")).toBe(true);
  for (const operation of cases.template.operations)
    expect(kinds[union!.selected(operation)]).toBe(operation.kind);
});
it("keys the template list on the worker session alone", () => {
  expect(listBuiltinTemplateMetadataOptions(null, 7).queryKey).toEqual([
    "finstack",
    7,
    "listBuiltinTemplateMetadata",
  ]);
});
function Connected({
  defaultJson,
  onSpecChange,
}: {
  defaultJson: string;
  onSpecChange: (json: string | null) => void;
}) {
  const templates = useListBuiltinTemplateMetadata();
  return (
    <MarketScenarioBuilder
      defaultJson={defaultJson}
      validate={useScenarioSpecValidator()}
      onSpecChange={onSpecChange}
      curveIds={cases.curveIds}
      templates={templates.data}
      templatesError={templates.error?.message}
      buildFromTemplate={useBuildFromTemplate()}
      buildTemplateComponent={useBuildTemplateComponent()}
      composeScenarios={useComposeScenarios()}
      buildMarketScenario={useBuildMarketScenario()}
    />
  );
}
const latest = (spy: ReturnType<typeof vi.fn>) =>
  spy.mock.calls.at(-1)?.[0] as string | null | undefined;
async function settled(spy: ReturnType<typeof vi.fn>, expected: string) {
  await waitFor(() => expect(latest(spy)).toBe(expected), {
    timeout: budget(8000),
  });
}
async function choose(label: string, option: string) {
  await userEvent.click(screen.getByRole("combobox", { name: label }));
  await userEvent.click(await screen.findByRole("option", { name: option }));
}
it(
  "loads a template component, composes another into it and shows the native canonical spec",
  async () => {
    const onSpecChange = vi.fn();
    render(
      <FinstackQueryProvider>
        <Connected
          defaultJson={JSON.stringify({ id: "s", operations: [] })}
          onSpecChange={onSpecChange}
        />
      </FinstackQueryProvider>,
    );
    await settled(onSpecChange, canonical({ id: "s", operations: [] }));
    const picker = screen.getByRole("region", { name: "Scenario templates" });
    await waitFor(
      () =>
        expect(
          (
            within(picker).getByRole("combobox", {
              name: "Template",
            }) as HTMLButtonElement
          ).disabled,
        ).toBe(false),
      { timeout: budget(5000) },
    );
    await choose("Template", "2022 Fed Hiking Cycle");
    const details = screen.getByLabelText("Template details");
    expect(details.textContent).toContain("2022-06-15");
    expect(details.textContent).toContain("moderate");
    await choose("Component", cases.componentIds.rates);
    await userEvent.click(
      screen.getByRole("button", { name: "Load template" }),
    );
    await settled(onSpecChange, canonical(cases.rates));
    // The placeholder curve id is free text in an IdCombobox with market suggestions.
    const field = document.querySelector<HTMLElement>(
      '[data-field-path="operations[0].curve_id"]',
    )!;
    const curveId = within(field).getByRole("combobox", { name: "Curve id" });
    expect((curveId as HTMLInputElement).value).toBe("USD-SOFR");
    fireEvent.change(curveId, { target: { value: "USD-OIS" } });
    await settled(onSpecChange, canonical(cases.remapped));
    await choose("Component", cases.componentIds.equity);
    await userEvent.click(
      screen.getByRole("button", { name: "Compose with scenario" }),
    );
    await settled(onSpecChange, canonical(cases.composed));
    expect(calls("composeScenarios")).toEqual([
      [
        "composeScenarios",
        [canonical(cases.remapped), JSON.stringify(cases.equity)],
      ],
    ]);
    const viewer = screen.getByRole("region", { name: "Scenario spec" });
    expect(viewer.textContent).toContain(cases.composed.id);
  },
  budget(30000),
);
it(
  "appends the operations Rust builds for a parallel shift and a roll, and shows a native rejection",
  async () => {
    const onSpecChange = vi.fn();
    render(
      <FinstackQueryProvider>
        <Connected
          defaultJson={JSON.stringify({ id: "s", operations: [] })}
          onSpecChange={onSpecChange}
        />
      </FinstackQueryProvider>,
    );
    await settled(onSpecChange, canonical({ id: "s", operations: [] }));
    const shift = screen.getByRole("group", { name: "Parallel shift" });
    await userEvent.click(
      within(shift).getByRole("button", { name: "Add parallel shift" }),
    );
    // A blank size is NaN: Rust rejects it and the document is unchanged.
    expect((await screen.findByRole("alert")).textContent).toBeTruthy();
    expect(latest(onSpecChange)).toBe(canonical({ id: "s", operations: [] }));
    await userEvent.click(
      within(shift).getByRole("combobox", { name: "Curve kind" }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "par_cds" }),
    );
    fireEvent.change(
      within(shift).getByRole("combobox", { name: "Curve id" }),
      {
        target: { value: "HZ" },
      },
    );
    fireEvent.change(
      within(shift).getByRole("textbox", { name: "Shift size" }),
      {
        target: { value: "-12.5" },
      },
    );
    await userEvent.click(
      within(shift).getByRole("button", { name: "Add parallel shift" }),
    );
    const shifted = {
      id: "s",
      operations: native.parallelBpMany("par_cds", ["HZ"], -12.5),
    };
    await settled(onSpecChange, canonical(shifted));
    const roll = screen.getByRole("group", { name: "Time roll" });
    fireEvent.change(
      within(roll).getByRole("textbox", { name: "Roll period" }),
      {
        target: { value: "3M" },
      },
    );
    await userEvent.click(
      within(roll).getByRole("button", { name: "Add roll" }),
    );
    await settled(
      onSpecChange,
      canonical({
        ...shifted,
        operations: [
          ...shifted.operations,
          native.operationSpecTimeRollForward("3M"),
        ],
      }),
    );
    // The builder sent the document's id and no roll mode, leaving Rust's default.
    expect(calls("buildMarketScenario").at(-1)).toEqual([
      "buildMarketScenario",
      { id: "s", shifts: [], roll: { period: "3M", rollMode: null } },
    ]);
  },
  budget(30000),
);
