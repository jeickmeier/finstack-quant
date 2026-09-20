// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { beforeAll, afterAll, afterEach, it, expect, vi } from "vitest";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  cleanup,
} from "@testing-library/react";
import { CalibrationForm } from "../registry/components/calibration-form/calibration-form";
import { calibrationModule } from "../registry/components/calibration-form/calibration";
import { startWorker } from "./worker/harness.mjs";
import { unwrap } from "../registry/workers/finstack-contract";
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const root = resolve(dirname(fileURLToPath(import.meta.url)), "../.."),
  load = (name: string) =>
    JSON.parse(
      readFileSync(
        resolve(
          root,
          `finstack-quant/calibration/examples/market_bootstrap/${name}.json`,
        ),
        "utf8",
      ),
    );
let worker: Awaited<ReturnType<typeof startWorker>>;
beforeAll(async () => {
  worker = await startWorker();
}, 30000);
afterAll(async () => worker?.close());
afterEach(cleanup);
const validate = async (json: string) =>
  unwrap(await worker.proxy.validateCalibration(json));
it("edits generated quotes, prior markets and settings and submits the exact native canonical envelope", async () => {
  const initial = load("01_usd_discount");
  initial.prior_market = load("05_cdx_base_correlation").prior_market;
  initial.plan.settings.compute_diagnostics = false;
  const onSubmit = vi.fn();
  render(
    <CalibrationForm
      defaultJson={JSON.stringify(initial)}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  const field = (path: string) =>
    document.querySelector(
      `[data-field-path="${path}"] input`,
    ) as HTMLInputElement;
  expect(field("market_data[0].rate")).toBeTruthy();
  fireEvent.change(field("market_data[0].rate"), {
    target: { value: "0.0527" },
  });
  fireEvent.change(field("prior_market[0].knot_points[1][1]"), {
    target: { value: "0.0013" },
  });
  const checkbox = document.querySelector(
    '[data-field-path="plan.settings.compute_diagnostics"] input[type="checkbox"]',
  ) as HTMLInputElement;
  expect(checkbox).toBeTruthy();
  fireEvent.click(checkbox);
  await waitFor(
    () =>
      expect(
        (screen.getByRole("button", { name: "Calibrate" }) as HTMLButtonElement)
          .disabled,
      ).toBe(false),
    { timeout: 5000 },
  );
  fireEvent.click(screen.getByRole("button", { name: "Calibrate" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalled(), { timeout: 5000 });
  initial.market_data[0].rate = 0.0527;
  initial.prior_market[0].knot_points[1][1] = 0.0013;
  initial.plan.settings.compute_diagnostics = true;
  expect(onSubmit.mock.calls.at(-1)![0]).toBe(
    native.validateCalibrationJson(JSON.stringify(initial)),
  );
  expect(
    calibrationModule.codec.stringify(
      calibrationModule.codec.parse(onSubmit.mock.calls[0]![0]),
    ),
  ).toBeTruthy();
}, 15000);
it("keeps native semantic failures in the summary and prevents calibration acceptance", async () => {
  const initial = load("01_usd_discount");
  initial.plan.steps[0].quote_set = "missing_quotes";
  const onSubmit = vi.fn();
  render(
    <CalibrationForm
      defaultJson={JSON.stringify(initial)}
      validate={validate}
      onSubmit={onSubmit}
    />,
  );
  let message;
  try {
    native.validateCalibrationJson(JSON.stringify(initial));
  } catch (error) {
    message = (error as Error).message;
  }
  await waitFor(() =>
    expect(
      screen
        .getAllByRole("alert")
        .some((node) => node.textContent?.includes(message!)),
    ).toBe(true),
  );
  fireEvent.click(screen.getByRole("button", { name: "Calibrate" }));
  await waitFor(() =>
    expect(screen.getAllByRole("alert").length).toBeGreaterThan(0),
  );
  expect(onSubmit).not.toHaveBeenCalled();
}, 15000);

it.each(["null", "[]", "42", '"text"', "{ malformed"])(
  "rejects invalid initial root %s instead of loading an example",
  (json) => {
    const onSubmit = vi.fn();
    render(
      <CalibrationForm
        defaultJson={json}
        validate={validate}
        onSubmit={onSubmit}
      />,
    );
    expect(screen.getByRole("alert")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Calibrate" })).toBeNull();
    expect(onSubmit).not.toHaveBeenCalled();
  },
);

it("exposes structurally valid JSON for diagnostics while native rejection prevents acceptance, and clears it on edit", async () => {
  const input = vi.fn(),
    accepted = vi.fn();
  render(
    <CalibrationForm
      defaultJson={JSON.stringify(load("01_usd_discount"))}
      validate={async () => {
        throw new Error("Rejected plan");
      }}
      onSubmit={() => {}}
      onValidated={accepted}
      onInputJson={input}
    />,
  );
  await waitFor(
    () =>
      expect(input.mock.calls.some(([json]) => typeof json === "string")).toBe(
        true,
      ),
    { timeout: 5000 },
  );
  expect(accepted.mock.calls.every(([json]) => json === null)).toBe(true);
  input.mockClear();
  const field = document.querySelector(
    '[data-field-path="market_data[0].rate"] input',
  ) as HTMLInputElement;
  fireEvent.change(field, { target: { value: "not-a-number" } });
  expect(input).toHaveBeenCalledWith(null);
  await waitFor(() =>
    expect(screen.getAllByRole("alert").length).toBeGreaterThan(0),
  );
  expect(input.mock.calls.every(([json]) => json === null)).toBe(true);
});
