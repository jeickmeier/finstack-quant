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
    await workbench.getByRole("tab", { name: "Request", exact: true }).click();
    const requestViewer = workbench.getByRole("region", {
      name: "Last priced request JSON",
      exact: true,
    });
    await requestViewer
      .getByRole("button", { name: "Original", exact: true })
      .click();
    const request = JSON.parse(
      await requestViewer.locator("pre").textContent(),
    );
    await workbench
      .getByRole("tab", { name: "3 Cashflows", exact: true })
      .click();
    await workbench.getByRole("tab", { name: "2 Market", exact: true }).click();
    await workbench.getByLabel("Search market fields").fill("/curves/0");
    await workbench
      .getByRole("button", { name: "Inspect /curves/0", exact: true })
      .click();
    for (const format of ["A4", "Letter"]) {
      await page.evaluate(() => {
        window.readPrintFigureGeometry = () =>
          [...document.querySelectorAll("[data-printing] svg.ts-chart")].map(
            (svg) => {
              const box = svg.getBoundingClientRect(),
                viewBox = svg.viewBox.baseVal,
                matrix = svg.getScreenCTM();
              return {
                width: box.width,
                height: box.height,
                viewBoxWidth: viewBox.width,
                viewBoxHeight: viewBox.height,
                scaleX: matrix ? Math.hypot(matrix.a, matrix.b) : null,
                scaleY: matrix ? Math.hypot(matrix.c, matrix.d) : null,
              };
            },
          );
        window.print = () => {
          window.printFigureGeometry = window.readPrintFigureGeometry();
          window.printPrepared = true;
        };
        window.printPrepared = false;
      });
      await workbench
        .getByRole("button", { name: "Print report", exact: true })
        .click();
      await page.waitForFunction(() => window.printPrepared === true);
      await expect(workbench).toHaveAttribute("data-printing", "ready");
      const preparedFigures = await page.evaluate(
        () => window.printFigureGeometry,
      );
      assert.equal(
        preparedFigures.length,
        1,
        "Selected market curve remains in the report",
      );
      for (const figure of preparedFigures) {
        assert(
          figure.width >= 640 && figure.width <= 690,
          "Prepared market figure has a stable paper-sized width",
        );
        assert(
          figure.height <= 260,
          "Prepared market figure retains its compact physical height",
        );
        assert(
          Math.abs(figure.scaleX - 1) < 0.02 &&
            Math.abs(figure.scaleY - 1) < 0.02,
          "Prepared SVG text is not stretched",
        );
      }
      for (const name of ["Priced instrument JSON", "Market snapshot"])
        await workbench
          .getByRole("region", { name, exact: true })
          .getByRole("button", { name: "Original", exact: true })
          .click();
      const instrument = await workbench
        .locator('[aria-label="Priced instrument JSON"] pre')
        .textContent();
      const market = await workbench
        .locator('[aria-label="Market snapshot"] pre')
        .textContent();
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
      const schedule = cashflows.getByRole("table", {
        name: "Cashflow schedule",
        exact: true,
      });
      await expect(schedule).toBeVisible();
      await expect(schedule.locator("tbody tr")).toHaveCount(
        JSON.parse(expected).flows.length,
      );
      await expect(cashflows.locator("pre")).toBeHidden();
      await cashflows.getByText("Original JSON", { exact: true }).click();
      await cashflows
        .getByRole("button", { name: "Original", exact: true })
        .click();
      assert.equal(await cashflows.locator("pre").textContent(), expected);
      const downloadPromise = page.waitForEvent("download");
      await cashflows
        .getByRole("button", { name: "Download", exact: true })
        .click();
      assert.equal(
        await readFile(await (await downloadPromise).path(), "utf8"),
        expected,
      );
      for (const viewer of [
        workbench.getByRole("region", {
          name: "Priced instrument JSON",
          exact: true,
        }),
        workbench.getByRole("region", { name: "Market snapshot", exact: true }),
      ])
        await viewer
          .getByRole("button", { name: "Formatted", exact: true })
          .click();
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
      await expect(cashflows.locator("[data-json-print-source]")).toBeHidden();
      await expect(cashflows.locator("pre")).toBeHidden();
      await expect(schedule).toBeVisible();
      const cashflowRows = await schedule
        .locator("tbody tr")
        .evaluateAll((rows) =>
          rows.map((row) =>
            [...row.querySelectorAll("td")]
              .filter((cell) => getComputedStyle(cell).display !== "none")
              .map((cell) => cell.innerText),
          ),
        );
      const cashflowFooter = await schedule.locator("tfoot").innerText();
      const geometry = await schedule.evaluate((node) => ({
        overflow: node.scrollWidth > node.clientWidth,
        rows: node.querySelectorAll("tbody tr").length,
      }));
      assert.equal(
        geometry.overflow,
        false,
        "Printed table must fit its paper width",
      );
      assert.equal(geometry.rows, JSON.parse(expected).flows.length);
      const printedFigures = await page.evaluate(() =>
        window.readPrintFigureGeometry(),
      );
      assert.equal(printedFigures.length, preparedFigures.length);
      printedFigures.forEach((figure, index) => {
        assert(
          Math.abs(figure.width - preparedFigures[index].width) < 1,
          "Print media preserves the prepared chart width",
        );
        assert(
          Math.abs(figure.height - preparedFigures[index].height) < 1,
          "Print media preserves the prepared chart height",
        );
        assert(
          Math.abs(figure.scaleX - 1) < 0.02 &&
            Math.abs(figure.scaleY - 1) < 0.02,
          "Print SVG text is not stretched",
        );
      });
      const filename = `${kind}-${format}.pdf`;
      const measureValues = await workbench
        .locator(".finstack-measures tbody td:nth-child(2) > span[title]")
        .evaluateAll((spans) =>
          spans.map((span) => span.getAttribute("title")),
        );
      if (kind === "bond")
        assert(
          measureValues.length > 0,
          "Bond report must contain supplied measures",
        );
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
        JSON.stringify(
          { instrument, market, cashflowRows, cashflowFooter, measureValues },
          null,
          2,
        ) + "\n",
      );
      await page.emulateMedia({ media: "screen" });
      await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
      await expect(workbench).not.toHaveAttribute("data-printing");
      // Printing restores the disclosure state; close the source before the
      // next format so its default table check starts from the same UI state.
      await cashflows.getByText("Original JSON", { exact: true }).click();
      await expect(cashflows.locator("pre")).toBeHidden();
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
        preparedFigures,
        printedFigures,
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
    "A4/Letter bond and mixed-currency reports print complete cashflow tables; exact native downloads; standalone publication SVG/PNG",
  ];
}
