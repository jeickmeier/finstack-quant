// @vitest-environment jsdom
import { useEffect } from "react";
import { it, expect, vi, afterEach } from "vitest";
import {
  render,
  screen,
  fireEvent,
  act,
  cleanup,
} from "@testing-library/react";
import { CalibrationPanel } from "@/components/finstack/valuations/blocks/pricing-workbench/calibration-panel";
import cases from "../calibration/cases.json";
const pending = vi.hoisted(() => ({ validate: vi.fn() }));
vi.mock("@/hooks/core/use-market-validator/use-market-validator", () => ({
  useMarketValidator: () => pending.validate,
}));
vi.mock("@/hooks/calibration/use-calibrate/use-calibrate", () => ({
  useCalibrationValidator: () => async (json: string) => json,
  useCalibrationDryRun: () => ({}),
  useCalibrate: (request: string | null) => ({
    data: request ? cases[0]!.result : undefined,
  }),
}));
// Form validation and reports have native component/browser coverage; isolate late handoff orchestration here.
vi.mock(
  "@/components/finstack/calibration/components/calibration-form/calibration-form",
  () => ({
    CalibrationForm: ({ defaultJson, onValidated, onSubmit }: any) => {
      useEffect(() => {
        onValidated(defaultJson);
      }, [defaultJson, onValidated]);
      return (
        <>
          <button onClick={() => onSubmit(defaultJson)}>Submit fixture</button>
          <button onClick={() => onValidated(null)}>Invalidate fixture</button>
          <button onClick={() => onValidated(defaultJson)}>
            Accept fixture
          </button>
        </>
      );
    },
  }),
);
vi.mock(
  "@/components/finstack/calibration/components/calibration-report/calibration-report",
  () => ({
    CalibrationReport: () => null,
  }),
);
vi.mock(
  "@/components/finstack/calibration/components/calibration-fit-chart/calibration-fit-chart",
  () => ({ CalibrationFitChart: () => null }),
);
afterEach(cleanup);
it("rejects a late market validation after input changes, but accepts the matching current solve", async () => {
  let finish!: (json: string) => void;
  pending.validate.mockImplementation(
    () =>
      new Promise<string>((resolve) => {
        finish = resolve;
      }),
  );
  const accepted = vi.fn();
  render(
    <CalibrationPanel
      defaultJson={JSON.stringify(cases[0]!.input)}
      onMarket={accepted}
    />,
  );
  fireEvent.click(screen.getByText("Submit fixture"));
  const apply = screen.getByText("Use calibrated market") as HTMLButtonElement;
  fireEvent.click(apply);
  expect(JSON.parse(pending.validate.mock.calls[0]![0])).toEqual(
    cases[0]!.result.result.final_market,
  );
  fireEvent.click(screen.getByText("Invalidate fixture"));
  await act(async () => finish("old canonical market"));
  expect(accepted).not.toHaveBeenCalled();
  expect(apply.disabled).toBe(true);
  fireEvent.click(screen.getByText("Accept fixture"));
  fireEvent.click(apply);
  await act(async () => finish("current canonical market"));
  expect(accepted).toHaveBeenCalledExactlyOnceWith("current canonical market");
});
