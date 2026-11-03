import { test, expect } from "playwright/test";
import data from "../../../docs-site/src/components/registry-gallery/data.json" with { type: "json" };

const fixture = data.portfolioWorkbench;

test("portfolio-workbench opens each tab on the native result, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=portfolio-workbench&theme=light&density=compact&focus=1",
  );
  const workbench = page.getByRole("region", { name: "Portfolio workbench" });
  await expect(
    workbench
      .getByRole("region", { name: "Portfolio summary" })
      .getByTitle(fixture.report.valuation.total_base_currency.amount)
      .first(),
  ).toBeVisible();
  await workbench.getByRole("tab", { name: "Risk" }).click();
  await expect(
    workbench.getByRole("region", { name: "Portfolio metrics" }),
  ).toBeVisible();
  await workbench.getByRole("tab", { name: "Scenarios" }).click();
  const pnl = workbench.getByRole("region", { name: "Portfolio scenario P&L" });
  for (const item of fixture.batch.items)
    await expect(pnl.getByTitle(item.pnl.total.amount).first()).toBeVisible();
  await workbench.getByRole("tab", { name: "Margin" }).click();
  await expect(
    workbench
      .getByRole("region", { name: "Portfolio margin" })
      .getByTitle(fixture.margin.total_margin.amount)
      .first(),
  ).toBeVisible();
  await workbench.getByRole("tab", { name: "Book" }).click();
  const editor = workbench.getByRole("textbox", {
    name: "PortfolioSpec JSON",
  });
  await editor.fill("{");
  await workbench.getByRole("button", { name: "Validate & value" }).click();
  await expect(workbench.getByRole("alert")).not.toHaveText("");
  await expect(workbench.getByRole("alert")).toHaveCount(1);
  expect(errors).toEqual([]);
});
