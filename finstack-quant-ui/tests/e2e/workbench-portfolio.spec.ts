import { test, expect } from "playwright/test";
import data from "../../../docs-site/src/components/registry-gallery/data.json" with { type: "json" };

const fixture = data.portfolioWorkbench;
const returns = data.portfolioReturns.cases;
const brinson = data.portfolioBrinson.result;
const exposures = data.portfolioExposures;
const attribution = data.portfolioAttribution.cases.find(
  (entry) => entry.scenario === "usd-ois-50bp" && entry.name === "taylor",
)!;

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
  const primitives = workbench.getByRole("region", {
    name: "Primitive exposures",
  });
  for (const value of [
    exposures.report.aggregates[1]!.gross_value.amount,
    exposures.report.aggregates[0]!.gross_measures.dv01,
    exposures.sensitivities.data[0]![0]!,
  ])
    await expect(primitives.getByTitle(String(value)).first()).toBeVisible();
  await expect(
    primitives.getByRole("img", { name: "Net and gross value by primitive" }),
  ).toBeVisible();
  await workbench.getByRole("tab", { name: "Scenarios" }).click();
  const pnl = workbench.getByRole("region", { name: "Portfolio scenario P&L" });
  for (const item of fixture.batch.items)
    await expect(pnl.getByTitle(item.pnl.total.amount).first()).toBeVisible();
  await workbench.getByRole("tab", { name: "Attribution" }).click();
  await workbench.getByRole("radio", { name: "taylor" }).click();
  await expect(
    workbench
      .getByRole("region", { name: "Portfolio attribution" })
      .getByTitle(attribution.report!.attribution.total_pnl.amount)
      .first(),
  ).toBeVisible();
  await workbench.getByRole("tab", { name: "Returns" }).click();
  const performance = workbench.getByRole("region", {
    name: "Portfolio returns",
  });
  for (const value of [
    returns.linked.annual.result.cumulative,
    returns.dietz.contribution.result,
    returns.xirr.conventional.result,
  ])
    await expect(performance.getByTitle(String(value))).toBeVisible();
  await workbench.getByRole("tab", { name: "Brinson" }).click();
  const linked = workbench.getByRole("region", {
    name: "Brinson attribution",
  });
  for (const value of [
    brinson.portfolio_return_compounded,
    brinson.linked_allocation,
    brinson.periods[2]!.total_excess_return,
  ])
    await expect(linked.getByTitle(String(value)).first()).toBeVisible();
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
