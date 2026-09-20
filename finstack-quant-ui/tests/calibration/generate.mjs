import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const inventory = JSON.parse(
  await readFile(
    new URL("../../src/generated/fixtures.json", import.meta.url),
    "utf8",
  ),
);
const cases = [];
for (const fixture of inventory.filter((f) => f.kind === "calibration-input")) {
  const json = await readFile(
    new URL(`../../../${fixture.source}`, import.meta.url),
    "utf8",
  );
  cases.push({
    source: fixture.source,
    sha256: createHash("sha256").update(json).digest("hex"),
    input: JSON.parse(native.validateCalibrationJson(json)),
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
await writeFile(
  new URL("./cases.json", import.meta.url),
  JSON.stringify(cases, null, 2) + "\n",
);

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
  await writeFile(
    new URL("./failure.json", import.meta.url),
    JSON.stringify(
      Object.fromEntries(
        Object.getOwnPropertyNames(error)
          .filter((key) => key !== "stack" && error[key] !== undefined)
          .map((key) => [key, error[key]]),
      ),
      null,
      2,
    ) + "\n",
  );
}
