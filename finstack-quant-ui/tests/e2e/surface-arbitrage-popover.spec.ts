import { test, expect } from "playwright/test";

// The gallery baselines capture the preview only; the popover is portaled
// outside it, so this screenshots the open violations popover on its own.
test("surface-arbitrage-badge violations popover", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=surface-arbitrage-badge&theme=dark&density=compact&focus=1",
  );
  const row = page
    .locator("[data-arbitrage-status]")
    .filter({ hasText: "6 violations" });
  await expect(row).toBeVisible();
  await row.getByRole("button", { name: "Arbitrage details" }).click();
  const popover = page.getByRole("dialog");
  await expect(
    popover.getByRole("table", { name: "Violations by type" }),
  ).toContainText("Butterfly");
  await page.evaluate(() => document.fonts.ready);
  await expect(popover).toHaveScreenshot("surface-arbitrage-popover.png");
  expect(errors).toEqual([]);
});
