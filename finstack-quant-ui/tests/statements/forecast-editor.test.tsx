// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ForecastEditor } from "@/components/finstack/statements/blocks/statements-workbench/forecast-editor";
import { budget } from "../shared/budget";
import model from "../../src/fixtures/statements/analyst-model.json";

afterEach(cleanup);

const spec = model as never as Parameters<typeof ForecastEditor>[0]["model"];
const nodeId = Object.keys(spec.nodes).find(
  (id) => spec.nodes[id].forecast !== undefined,
)!;
const forecastPeriods = spec.periods.periods
  .filter((period) => !period.is_actual)
  .map((period) => period.id);

function show(forecast: object) {
  const onApply = vi.fn(async () => null as string | null);
  render(
    <ForecastEditor
      model={spec}
      nodeId={nodeId}
      node={{ ...spec.nodes[nodeId], forecast } as never}
      onApply={onApply}
    />,
  );
  return onApply;
}
async function apply() {
  const button = screen.getByRole<HTMLButtonElement>("button", {
    name: "Apply forecast",
  });
  await waitFor(() => expect(button.disabled).toBe(false));
  fireEvent.click(button);
}

it("submits the fields the Rust schema publishes", async () => {
  const onApply = show({ method: "growth_pct", params: { rate: 0.03 } });
  fireEvent.change(screen.getByRole("textbox", { name: /^Rate/ }), {
    target: { value: "0.05" },
  });
  await apply();
  await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
  const forecast = (onApply.mock.calls[0] as unknown[])[0] as object;
  expect(forecast).toEqual({ method: "growth_pct", params: { rate: 0.05 } });
});

it("offers the fade parameters and leaves Rust's defaults unset", async () => {
  const onApply = show({ method: "fade_to_target", params: { target: 10 } });
  expect(screen.getByRole("button", { name: "Add shape" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Add half life" })).toBeTruthy();
  await apply();
  await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
  expect((onApply.mock.calls[0] as unknown[])[0]).toEqual({
    method: "fade_to_target",
    params: { target: 10 },
  });
});

it("edits one rate per forecast period", async () => {
  const onApply = show({ method: "curve_pct", params: {} });
  const first = forecastPeriods[0]!;
  fireEvent.change(
    screen.getByRole("textbox", { name: `${first} growth rate` }),
    { target: { value: "0.2" } },
  );
  await apply();
  await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
  expect((onApply.mock.calls[0] as unknown[])[0]).toEqual({
    method: "curve_pct",
    params: { curve: forecastPeriods.map((_, i) => (i === 0 ? 0.2 : 0)) },
  });
});

it("sends only the scheduled values that were entered", async () => {
  const onApply = show({ method: "override", params: { overrides: {} } });
  const first = forecastPeriods[0]!;
  fireEvent.change(
    screen.getByRole("textbox", { name: `${first} scheduled value` }),
    { target: { value: "120" } },
  );
  await apply();
  await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
  expect((onApply.mock.calls[0] as unknown[])[0]).toEqual({
    method: "override",
    params: { overrides: { [first]: 120 } },
  });
});

it("reads comma-separated history as exact numbers", async () => {
  const onApply = show({
    method: "bootstrap",
    params: { historical: [1, 2], seed: 7 },
  });
  fireEvent.change(screen.getByRole("textbox", { name: /Historical/ }), {
    target: { value: "1.5, 2.5, 3" },
  });
  await apply();
  await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
  expect((onApply.mock.calls[0] as unknown[])[0]).toEqual({
    method: "bootstrap",
    params: { historical: [1.5, 2.5, 3], seed: 7 },
  });
});

it(
  "shows the error onApply returns and clears it when the method changes",
  { timeout: budget(10000) },
  async () => {
    const onApply = vi.fn(
      async () => "half_life is only legal with exponential",
    );
    render(
      <ForecastEditor
        model={spec}
        nodeId={nodeId}
        node={
          {
            ...spec.nodes[nodeId],
            forecast: { method: "growth_pct", params: { rate: 0.03 } },
          } as never
        }
        onApply={onApply}
      />,
    );
    await apply();
    expect((await screen.findByRole("alert")).textContent).toContain(
      "half_life",
    );
    await userEvent.click(
      screen.getByRole("combobox", { name: /forecast method/ }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Carry forward" }),
    );
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  },
);

it(
  "holds a required field's error until an edit or Apply",
  { timeout: budget(10000) },
  async () => {
    const onApply = show({ method: "growth_pct", params: { rate: 0.03 } });
    await userEvent.click(
      screen.getByRole("combobox", { name: /forecast method/ }),
    );
    await userEvent.click(
      await screen.findByRole("option", { name: "Fade to target" }),
    );
    const target = await screen.findByRole("textbox", { name: /^Target/ });
    const button = screen.getByRole<HTMLButtonElement>("button", {
      name: "Apply forecast",
    });
    // The native check has settled and the field is still empty.
    await waitFor(() => expect(button.disabled).toBe(false));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(target.getAttribute("aria-invalid")).not.toBe("true");

    fireEvent.click(button);
    await waitFor(() =>
      expect(target.getAttribute("aria-invalid")).toBe("true"),
    );
    expect(screen.getAllByRole("alert").length).toBeGreaterThan(0);
    expect(onApply).not.toHaveBeenCalled();

    fireEvent.change(target, { target: { value: "10" } });
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(target.getAttribute("aria-invalid")).not.toBe("true");
    await waitFor(() => expect(button.disabled).toBe(false));
    fireEvent.click(button);
    await waitFor(() => expect(onApply).toHaveBeenCalledTimes(1));
    expect((onApply.mock.calls[0] as unknown[])[0]).toEqual({
      method: "fade_to_target",
      params: { target: 10 },
    });
  },
);
