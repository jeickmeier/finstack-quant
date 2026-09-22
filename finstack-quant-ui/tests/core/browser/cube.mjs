import { createRequire } from "node:module";
import assert from "node:assert/strict";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";
import {
  installBuilt,
  buildConsumer,
  openConsumer,
  closeConsumer,
} from "../../browser/consumer.mjs";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const consumer = await mkdtemp(path.join(root, ".consumer-cube-"));
let browser, server, page;
try {
  const installed = await installBuilt(root, consumer, [
    "market-context-form",
    "use-market-validator",
    "vol-cube-explorer",
  ]);
  await writeFile(
    path.join(consumer, "main.tsx"),
    await readFile(new URL("./fixture/cube.tsx", import.meta.url)),
  );
  await writeFile(
    path.join(consumer, "fixture.json"),
    await readFile(path.join(root, "tests/core/cubes/cases.json")),
  );

  const modules = await buildConsumer(consumer, { title: "Cashflow exports" });
  assert(!modules.some((id) => /finstack-quant-wasm/.test(id)));
  ({ server, browser, page } = await openConsumer(path.join(consumer, "dist"), {
    viewport: { width: 1000, height: 1000 },
  }));
  const failures = [],
    requests = [];
  page.on("pageerror", (error) => failures.push(error.message));
  page.on("response", (r) => {
    requests.push(r.url());
    if (r.status() >= 400) failures.push(`${r.status()} ${r.url()}`);
  });
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"], {
    origin: server.url,
  });
  await page.goto(server.url, { waitUntil: "networkidle" });
  const native = createRequire(import.meta.url)(
    "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const id = "USD-SWAPTION-NORMAL-VOL";
  const values = [];
  async function compare(convention, strike) {
    const cube = await page.evaluate(() => window.cubeProbe.cube);
    const handle = new native.VolCube(
      cube.id,
      cube.expiries,
      cube.tenors,
      cube.params.flatMap((p) => [
        p.alpha,
        p.beta,
        p.rho,
        p.nu,
        p.shift ?? NaN,
      ]),
      cube.forwards,
      cube.interpolation_mode,
    );
    let expected;
    try {
      expected = cube.expiries.flatMap((expiry) =>
        cube.tenors.map((tenor) =>
          (convention === "normal"
            ? native.getCubeNormalVol
            : native.getCubeVol)(handle, expiry, tenor, strike),
        ),
      );
    } finally {
      handle.free();
    }
    await page.waitForFunction(
      ({ id, expected }) => {
        const rows = [
          ...document.querySelectorAll(
            `[aria-label="${id} evaluated surface"] tbody tr`,
          ),
        ];
        return (
          rows.length === expected.length &&
          rows.every(
            (row, i) =>
              row.querySelectorAll("td")[2].textContent === String(expected[i]),
          )
        );
      },
      { id: cube.id, expected },
    );
    values.push({ id: cube.id, convention, strike, expected });
    return expected;
  }
  await compare("normal", 0.05);
  const chart = page.locator(`[aria-label="${id} heatmap"][tabindex]`);
  await chart.focus();
  await chart.press("Home");
  await chart.press("Enter");
  await page.getByText("Absolute strike 0.05", { exact: true }).waitFor();
  const raw = page.getByRole("table", { name: "Raw cube nodes", exact: true });
  const expectedRaw = await page.evaluate(() => {
    const c = window.cubeProbe.cube;
    return c.expiries.flatMap((e, r) =>
      c.tenors.map((t, col) => {
        const i = r * c.tenors.length + col,
          p = c.params[i];
        return [
          e,
          t,
          c.forwards[i],
          p.alpha,
          p.beta,
          p.rho,
          p.nu,
          p.shift ?? "Not supplied",
        ].map(String);
      }),
    );
  });
  assert.deepEqual(
    await raw
      .locator("tbody tr")
      .evaluateAll((rows) =>
        rows.map((row) =>
          [...row.querySelectorAll("td")].map((cell) => cell.textContent),
        ),
      ),
    expectedRaw,
  );
  const exports = [];
  for (const convention of ["normal", "black_lognormal"]) {
    await page
      .getByRole("radio", {
        name: convention === "normal" ? "Normal/Bachelier" : "Black/lognormal",
        exact: true,
      })
      .check();
    await compare(convention, 0.05);
    const svg = await page.evaluate(() => window.cubeProbe.export());
    assert(
      svg.includes(
        convention === "normal"
          ? "Normal volatility (annualized absolute rate)"
          : "Black/lognormal volatility (annualized decimal)",
      ),
    );
    assert(
      svg.includes("Checked evaluator output") &&
        svg.includes("Canonical cube state"),
    );
    await writeFile(`/tmp/pr030-${convention}.svg`, svg);
    exports.push({ convention, bytes: Buffer.byteLength(svg) });
  }
  await page
    .getByLabel("Absolute strike (rate units)", { exact: true })
    .fill("0.051");
  await compare("black_lognormal", 0.051);
  await page.getByRole("button", { name: new RegExp(`Edit .*${id}`) }).click();
  const cubeIndex = await page.evaluate(() =>
    JSON.parse(
      document.querySelector('[aria-label="Market or calibration result JSON"]')
        .value,
    ).vol_cubes.findIndex((c) => c.id === "USD-SWAPTION-NORMAL-VOL"),
  );
  await page
    .locator(
      `[data-field-path="vol_cubes[${cubeIndex}].params[0].alpha"] input`,
    )
    .fill("0.008");
  await page
    .locator(`[data-field-path="vol_cubes[${cubeIndex}].forwards[0]"] input`)
    .fill("0.051");
  await page.getByRole("button", { name: "Apply market", exact: true }).click();
  await page.waitForFunction(
    () => window.cubeProbe.cube.params[0].alpha === 0.008,
  );
  await compare("black_lognormal", 0.051);
  await page.getByText("Toggle outside coordinate", { exact: true }).click();
  await page.getByRole("alert").waitFor();
  const cube = await page.evaluate(() => window.cubeProbe.cube),
    handle = new native.VolCube(
      cube.id,
      cube.expiries,
      cube.tenors,
      cube.params.flatMap((p) => [
        p.alpha,
        p.beta,
        p.rho,
        p.nu,
        p.shift ?? NaN,
      ]),
      cube.forwards,
      cube.interpolation_mode,
    );
  let nativeError;
  try {
    native.getCubeVol(handle, 999, 5, 0.051);
  } catch (e) {
    nativeError = e instanceof Error ? e.message : String(e);
  } finally {
    handle.free();
  }
  assert.equal(await page.getByRole("alert").textContent(), nativeError);
  await page.getByText("Toggle outside coordinate", { exact: true }).click();
  await compare("black_lognormal", 0.051);
  await page.getByText("Switch cube fixture", { exact: true }).click();
  await compare("black_lognormal", 0.051);
  await page
    .getByRole("radio", { name: "Normal/Bachelier", exact: true })
    .check();
  await compare("normal", 0.051);
  await page.addScriptTag({
    path: path.join(root, "node_modules/axe-core/axe.min.js"),
  });
  const accessibility = [];
  for (const theme of ["light", "dark"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    const axe = await page.evaluate(() => window.axe.run(document));
    accessibility.push({ theme, violations: axe.violations });
    await page
      .getByRole("region", { name: "SHIFTED-BLACK cube explorer", exact: true })
      .screenshot({ path: `/tmp/pr030-${theme}.png` });
  }
  assert(
    accessibility.every((a) => a.violations.length === 0),
    JSON.stringify(accessibility),
  );
  assert.deepEqual(failures, []);
  assert.equal(requests.filter((url) => url.endsWith(".wasm")).length, 1);
  const report = {
    browser: browser.version(),
    installed,
    values,
    exports,
    nativeError,
    accessibility,
    wasmRequests: 1,
    failures,
    verdict: "pass",
  };
  await writeFile(
    "/tmp/pr030-cube.json",
    JSON.stringify(report, null, 2) + "\n",
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await closeConsumer({ browser, server }, consumer);
}
