import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { expect } from "playwright/test";
export async function verifyPublishing(page, evidence, repo) {
  const native = createRequire(import.meta.url)(
    path.join(repo, "finstack-quant-wasm/pkg-node/finstack_quant_wasm.js"),
  );
  const tokens = JSON.parse(
    await readFile(
      path.join(
        repo,
        "finstack-quant-ui/registry/theme/finstack-theme/tokens.json",
      ),
      "utf8",
    ),
  );
  await page.evaluate(async () => {
    document.documentElement.dataset.theme = "dark";
    await document.fonts.ready;
    await new Promise((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(resolve)),
    );
  });
  const { svg, png } = await page.evaluate(() => window.exportPublication());
  assert(
    svg.includes(`fill="${tokens.light.background}"`),
    "Light publication background must be independent of the dark app",
  );
  assert(svg.includes("<text") && /<(path|rect|circle)/.test(svg));
  assert(!/foreignObject|<image|var\(/.test(svg));
  assert(svg.includes("data:font/") || svg.includes("data:application/"));
  const exportedText = await page.evaluate(
    (svg) =>
      [
        ...new DOMParser()
          .parseFromString(svg, "image/svg+xml")
          .querySelectorAll("text"),
      ]
        .map((node) => node.textContent)
        .join(" "),
    svg,
  );
  for (const phrase of [
    "Supplied observations with an explicit reference range",
    "A complete figure",
    "Illustrative supplied values",
    "Registry illustration",
    "Annotation values supplied by the caller",
    "Supplied coordinate",
    "Stored value (raw)",
    "Supplied series",
    "Figure note",
    "Supplied callout",
  ])
    assert(exportedText.includes(phrase), phrase);
  const image = Buffer.from(png);
  assert.equal(image.readUInt32BE(16), 1800);
  assert.equal(image.readUInt32BE(20), 1200);
  await writeFile(path.join(evidence, "publication.svg"), svg);
  await writeFile(path.join(evidence, "publication.png"), image);
  const reports = [];
  for (const [kind, label] of [
    ["bond", "Bond report"],
    ["xccy_swap", "Mixed currency report"],
  ]) {
    await page.getByRole("button", { name: label, exact: true }).click();
    const workbench = page.getByRole("region", {
      name: "Pricing workbench",
      exact: true,
    });
    await expect(workbench.getByText("Priced", { exact: true })).toBeVisible();
    await workbench.getByRole("tab", { name: "2 Market", exact: true }).click();
    await workbench.getByLabel("Search market fields").fill("/curves/0");
    await workbench
      .getByRole("button", { name: "Inspect /curves/0", exact: true })
      .click();
    for (const format of ["A4", "Letter"]) {
      await page.evaluate(() => {
        window.print = () => {
          window.printPrepared = true;
        };
        window.printPrepared = false;
      });
      await workbench
        .getByRole("button", { name: "Print report", exact: true })
        .click();
      await page.waitForFunction(() => window.printPrepared === true);
      await expect(workbench).toHaveAttribute("data-printing", "ready");
      const instrument = await workbench
        .locator('[aria-label="Priced instrument JSON"] pre')
        .textContent();
      const market = await workbench
        .locator('[aria-label="Market snapshot"] pre')
        .textContent();
      const request = JSON.parse(
        await workbench
          .locator('[aria-label="Last priced request JSON"] pre')
          .textContent(),
      );
      assert.equal(JSON.parse(instrument).instrument.type, kind);
      assert.equal(instrument, request.instrumentJson);
      assert.equal(market, request.marketJson);
      const expected = native.instrumentCashflowsJson(
        instrument,
        market,
        request.asOf,
        request.model ?? "default",
      );
      const cashflows = workbench.getByRole("region", {
        name: "Cashflows",
        exact: true,
      });
      assert.equal(await cashflows.locator("pre").textContent(), expected);
      const downloadPromise = page.waitForEvent("download");
      await cashflows
        .getByRole("button", { name: "Download", exact: true })
        .click();
      assert.equal(
        await readFile(await (await downloadPromise).path(), "utf8"),
        expected,
      );
      await page.emulateMedia({ media: "print" });
      await page.evaluate(
        () =>
          new Promise((resolve) =>
            requestAnimationFrame(() => requestAnimationFrame(resolve)),
          ),
      );
      assert.equal(
        await workbench.getByRole("button").filter({ visible: true }).count(),
        0,
      );
      await expect(cashflows.locator("pre")).toBeVisible();
      const geometry = await cashflows.locator("pre").evaluate((node) => ({
        wrap: getComputedStyle(node).whiteSpace,
        overflow: node.scrollWidth > node.clientWidth,
        maxHeight: getComputedStyle(node).maxHeight,
      }));
      assert.deepEqual(geometry, {
        wrap: "pre-wrap",
        overflow: false,
        maxHeight: "none",
      });
      const filename = `${kind}-${format}.pdf`;
      await page.pdf({
        path: path.join(evidence, filename),
        format,
        printBackground: true,
        margin: { top: "12mm", right: "12mm", bottom: "12mm", left: "12mm" },
        displayHeaderFooter: true,
        headerTemplate: "<span></span>",
        footerTemplate:
          '<div style="font-family:sans-serif;font-size:8px;width:100%;text-align:center"><span class="pageNumber"></span> / <span class="totalPages"></span></div>',
      });
      await writeFile(
        path.join(evidence, `${kind}-${format}-text.json`),
        JSON.stringify({ instrument, market, cashflows: expected }, null, 2) +
          "\n",
      );
      await page.emulateMedia({ media: "screen" });
      await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
      await expect(workbench).not.toHaveAttribute("data-printing");
      await expect(
        workbench.getByRole("tab", { name: "2 Market", exact: true }),
      ).toHaveAttribute("aria-selected", "true");
      reports.push({
        kind,
        format,
        filename,
        exactNativeText: true,
        exactDownload: true,
        controlsHidden: true,
        printGeometry: geometry,
      });
    }
  }
  await writeFile(
    path.join(evidence, "publishing.json"),
    JSON.stringify(
      {
        reports,
        figure: {
          layout: [900, 600],
          pixels: [1800, 1200],
          inches: [6, 4],
          ppiAtPlacement: 300,
          editableSvg: true,
          embeddedFonts: true,
          lightFromDark: true,
        },
      },
      null,
      2,
    ) + "\n",
  );
  return [
    "A4/Letter bond and mixed-currency reports reuse completed components; exact native text/downloads; standalone publication SVG/PNG",
  ];
}
