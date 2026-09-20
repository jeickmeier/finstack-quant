import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { expect } from "playwright/test";

export async function verifyItem(page, item, evidence, repo) {
  const checks = [];
  if (item.name === "cashflow-viewer") {
    const data = JSON.parse(
      await readFile(
        path.join(repo, "docs-site/src/components/registry-gallery/data.json"),
        "utf8",
      ),
    );
    const expected = data.cashflows.cases.find(
      (entry) => entry.type === "xccy_swap",
    ).cashflows;
    const viewer = page.getByRole("region", { name: "Cashflows", exact: true });
    await expect(
      viewer.getByRole("table", { name: "Cashflow schedule" }),
    ).toBeVisible();
    assert.equal(
      await viewer.locator("tbody tr").count(),
      JSON.parse(expected).flows.length,
    );
    assert.equal(await viewer.locator("details").getAttribute("open"), null);
    await viewer.getByText("Original JSON", { exact: true }).click();
    await viewer.getByRole("button", { name: "Original", exact: true }).click();
    await expect(viewer.locator("pre")).toHaveText(expected);
    assert.equal(await viewer.locator("pre").textContent(), expected);
    await page
      .context()
      .grantPermissions(["clipboard-read", "clipboard-write"]);
    await viewer.getByRole("button", { name: "Copy", exact: true }).click();
    await expect(viewer.getByRole("status")).toHaveText("Copied");
    assert.equal(
      await page.evaluate(() => navigator.clipboard.readText()),
      expected,
    );
    const downloadPromise = page.waitForEvent("download");
    await viewer.getByRole("button", { name: "Download", exact: true }).click();
    const download = await downloadPromise;
    assert.equal(download.suggestedFilename(), "cashflows.json");
    assert.equal(await readFile(await download.path(), "utf8"), expected);
    checks.push(
      "mixed-currency native text, copy and download match byte for byte",
    );
  }
  if (item.name === "finstack-chart") {
    const chart = page.locator(
      'svg.ts-chart[aria-label="Independent interactive observations"]',
    );
    await chart.focus();
    await page.keyboard.press("Home");
    await page.keyboard.press("Enter");
    await expect(
      page.getByLabel("Accepted selection", { exact: true }),
    ).toHaveText("a");
    await expect(page.getByLabel("Activations", { exact: true })).toHaveText(
      "1",
    );
    await expect(page.locator(".ts-chart-tooltip")).toHaveCount(2);
    await expect(chart.locator('circle[r="9"]')).toHaveCount(1);
    await expect(
      page.locator(
        'svg[aria-label="Independent linked comparison"] circle[r="9"]',
      ),
    ).toHaveCount(1);
    const action = page.getByRole("button", {
      name: "Open original detail",
      exact: true,
    });
    await action.focus();
    await page.keyboard.press("Enter");
    await expect(page.getByLabel("Detail", { exact: true })).toHaveText("a");
    await expect(page.getByLabel("Activations", { exact: true })).toHaveText(
      "1",
    );
    await page
      .getByRole("button", { name: "Select second observation" })
      .click();
    await expect(
      page.getByLabel("Accepted selection", { exact: true }),
    ).toHaveText("b");
    const linked = page.locator(
      'svg.ts-chart[aria-label="Independent linked comparison"]',
    );
    await linked.focus();
    await page.keyboard.press("End");
    await page.keyboard.press("Enter");
    await expect(
      page.getByLabel("Accepted selection", { exact: true }),
    ).toHaveText("c");
    await page
      .getByRole("button", { name: "Clear selection", exact: true })
      .click();
    await expect(
      page.getByLabel("Accepted selection", { exact: true }),
    ).toHaveText("none");
    await expect(chart.locator('circle[r="9"]')).toHaveCount(0);
    await expect(linked.locator('circle[r="9"]')).toHaveCount(0);
    const { svg, png } = await page.evaluate(() => window.installedFigure());
    assert(svg.includes("<text") && /<(path|circle|rect)/.test(svg));
    assert(!/foreignObject|<image|var\(/.test(svg));
    assert(/data:(font|application)\//.test(svg));
    const text = await page.evaluate(
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
      "Illustrative supplied values",
      "Registry illustration",
      "Figure note",
      "Supplied callout",
      "Supplied coordinate",
      "Stored value (raw)",
    ])
      assert(text.includes(phrase), phrase);
    const image = Buffer.from(png);
    assert.equal(image.readUInt32BE(16), 1800);
    assert.equal(image.readUInt32BE(20), 1200);
    await writeFile(path.join(evidence, "standalone.svg"), svg);
    await writeFile(path.join(evidence, "standalone.png"), image);
    checks.push(
      "native selection and cursor composition, custom tooltip action, callbacks, complete editable SVG and 1800x1200 PNG",
    );
  }
  if (
    [
      "curve-chart",
      "calibration-fit-chart",
      "scenario-heatmap",
      "vol-surface-chart",
      "fx-surface-chart",
      "vol-cube-explorer",
    ].includes(item.name)
  ) {
    const chart = page.locator("svg.ts-chart").first();
    await chart.focus();
    await page.keyboard.press("Home");
    await page.keyboard.press("Enter");
    await page.waitForFunction(() => window.wrapperProbe.activations === 1);
    const action = page.getByRole("button", {
      name: "Wrapper original detail",
      exact: true,
    });
    await action.focus();
    await page.keyboard.press("Enter");
    assert.deepEqual(await page.evaluate(() => window.wrapperProbe), {
      activations: 1,
      detail: true,
    });
    const { svg, png } = await page.evaluate(() =>
      window.exportInstalledWrapper(),
    );
    const text = await page.evaluate(
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
    assert(text.includes("Independent wrapper figure"));
    assert(text.includes("Supplied presentation through public wrapper props"));
    assert(svg.includes("<text") && /<(path|circle|rect)/.test(svg));
    assert(!/foreignObject|<image|var\(/.test(svg));
    const image = Buffer.from(png);
    assert.equal(image.readUInt32BE(16), 1800);
    assert.equal(image.readUInt32BE(20), 1200);
    await writeFile(path.join(evidence, `${item.name}.svg`), svg);
    checks.push(
      "public wrapper presentation, activation and custom tooltip props; editable SVG and PNG export handle",
    );
  }
  return checks;
}
