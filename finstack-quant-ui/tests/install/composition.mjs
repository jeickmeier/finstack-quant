import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { chromium } from "playwright";
import { expect } from "playwright/test";
import { serveExport } from "../browser/static-server.mjs";
const server = await serveExport(
    process.env.REGISTRY_EXPORT_DIR,
    process.env.NEXT_PUBLIC_BASE_PATH || "",
  ),
  browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage(),
    errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("response", (response) => {
    if (response.status() >= 400)
      errors.push(`${response.status()} ${response.url()}`);
  });
  await page.goto(`${server.url}/registry-selection/`, {
    waitUntil: "networkidle",
  });
  const group = page.getByRole("region", {
      name: "Linked stored curves",
      exact: true,
    }),
    status = group.locator("[data-link-status]");
  await group.getByRole("gridcell", { name: "OVERLAY", exact: true }).click();
  await expect(status).toContainText(
    'Accepted: ["series","OVERLAY"]; proposals: 1; activations: 1',
  );
  const value = group.getByRole("gridcell", {
    name: "0.9512345678901234",
    exact: true,
  });
  await value.click();
  await expect(value).toHaveAttribute("aria-selected", "true");
  await group
    .getByRole("button", { name: "Inspect OVERLAY", exact: true })
    .click();
  await expect(status).toContainText(
    "proposals: 2; activations: 2; detail: Inspect OVERLAY",
  );
  await group.getByRole("button", { name: "Clear", exact: true }).click();
  const chart = group.locator("svg.ts-chart").first();
  await chart.focus();
  await chart.press("End");
  await chart.press("Enter");
  await expect(status).toContainText(
    'Accepted: ["point","USD-OIS",40]; proposals: 4; activations: 3',
  );
  await expect(group.locator('td[aria-selected="true"]')).toHaveCount(1);
  await chart.press("Escape");
  await group.getByRole("checkbox", { name: "Reject proposals" }).check();
  await value.click();
  await expect(value).toHaveAttribute("aria-selected", "false");
  await expect(status).toContainText(
    'Accepted: ["point","USD-OIS",40]; proposals: 5; activations: 4',
  );
  await group.getByRole("checkbox", { name: "Reject proposals" }).uncheck();
  const select = group.getByRole("button", { name: "Select overlay point" });
  await select.click();
  await expect(select).toBeFocused();
  await group.getByRole("button", { name: "Reverse rows" }).click();
  await expect(value).toHaveAttribute("aria-selected", "true");
  assert.deepEqual(errors, []);
  await writeFile(
    process.env.REGISTRY_SELECTION_REPORT,
    JSON.stringify(
      {
        verdict: "pass",
        basePath: process.env.NEXT_PUBLIC_BASE_PATH || "",
        checks: [
          "row, cell and point selection in both directions",
          "nested controls do not activate rows",
          "caller rejection and explicit clear",
          "programmatic updates retain focus",
          "semantic identity survives reorder",
        ],
        errors,
      },
      null,
      2,
    ) + "\n",
  );
} finally {
  await browser.close();
  await server.close();
}
