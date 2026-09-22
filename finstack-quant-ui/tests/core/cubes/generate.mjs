import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
const source =
  "finstack-quant/calibration/examples/market_bootstrap/07_swaption_vol_surface.json";
const input = await readFile(
  new URL(`../../../../${source}`, import.meta.url),
  "utf8",
);
const market = native.calibrate(input).result.final_market;
const cube = market.vol_cubes.find((c) => c.id === "USD-SWAPTION-NORMAL-VOL");
if (!cube)
  throw new Error(
    "Native calibration did not produce its declared volatility cube",
  );
const shifted = {
  id: "SHIFTED-BLACK",
  expiries: [1, 2],
  tenors: [2, 5],
  params: Array.from({ length: 4 }, () => ({
    alpha: 0.03,
    beta: 0.5,
    rho: -0.2,
    nu: 0.4,
    shift: 0.03,
  })),
  forwards: [-0.005, 0.001, 0.002, 0.01],
  interpolation_mode: "vol",
};
const handle = new native.Market(
  JSON.stringify({ ...market, vol_cubes: [cube, shifted] }),
);
try {
  await writeFile(
    new URL("./cases.json", import.meta.url),
    JSON.stringify(
      {
        source,
        sha256: createHash("sha256").update(input).digest("hex"),
        market: JSON.parse(handle.toJson()),
      },
      null,
      2,
    ) + "\n",
  );
} finally {
  handle.free();
}
