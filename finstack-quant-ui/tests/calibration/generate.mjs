import { emit, sha256 } from "../../scripts/fixture-output.mjs";
import {
  native,
  readRepoJson,
  readRepoText,
} from "../../scripts/fixture-inputs.mjs";
const inventory = await readRepoJson(
  "finstack-quant-ui/src/generated/fixtures.json",
);
const cases = [];
for (const fixture of inventory.filter((f) => f.kind === "calibration-input")) {
  const json = await readRepoText(fixture.source);
  cases.push({
    source: fixture.source,
    sha256: sha256(json),
    input: native.validateCalibration(json),
    result: native.calibrate(json),
  });
}
const diagnosticInput = structuredClone(cases[0].input);
diagnosticInput.plan.settings.compute_diagnostics = true;
cases.push({
  source: cases[0].source,
  sha256: cases[0].sha256,
  variant: "diagnostics-enabled",
  input: diagnosticInput,
  result: native.calibrate(JSON.stringify(diagnosticInput)),
});
await emit(new URL("./cases.json", import.meta.url), JSON.stringify(cases));

const solver = structuredClone(
  cases.find((c) => c.source.includes("08_equity")).input,
);
solver.plan.steps[1].target_strikes = [140, 180, 220];
solver.plan.settings.fail_on_bad_fit = true;
solver.plan.settings.vol_surface = { validation_tolerance: 0.001 };
try {
  native.calibrate(JSON.stringify(solver));
  throw new Error("Expected solver failure");
} catch (error) {
  if (error.kind !== "solver_not_converged") throw error;
  await emit(
    new URL("./failure.json", import.meta.url),
    JSON.stringify(
      Object.fromEntries(
        Object.getOwnPropertyNames(error)
          .filter((key) => key !== "stack" && error[key] !== undefined)
          .map((key) => [key, error[key]]),
      ),
    ),
  );
}
