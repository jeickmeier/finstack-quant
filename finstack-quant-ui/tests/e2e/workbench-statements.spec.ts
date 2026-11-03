import { test, expect } from "playwright/test";

test("statements-workbench exports the result as a table and a CSV of those cells, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=statements-workbench&theme=light&density=compact&focus=1",
  );
  await page.getByText("Export table and CSV", { exact: true }).click();
  const region = page.getByRole("region", { name: "Statement export" });
  // A real worker delivers a missing value as NaN; it never shows as NaN.
  await expect(region.getByText("Undefined").first()).toBeVisible();
  await expect(region.getByText("NaN", { exact: true })).toHaveCount(0);
  const saved = page.waitForEvent("download");
  await region.getByRole("button", { name: "Download CSV" }).click();
  const file = await saved;
  expect(file.suggestedFilename()).toBe("statement-wide.csv");
  const stream = await file.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(chunk as Buffer);
  const csv = Buffer.concat(chunks).toString("utf8");
  const [header, first] = csv.split("\r\n");
  const names = header!.split(",");
  const cells = first!.split(",");
  expect(names[0]).toBe("period_id");
  expect(cells[names.indexOf("revenue_growth_qoq")]).toBe("");
  expect(csv).not.toContain("NaN");
  await region.getByRole("radio", { name: "Long" }).click();
  await expect(
    region.getByText("Statement result, long layout"),
  ).toBeAttached();
  expect(errors).toEqual([]);
});

test("statements-workbench WACC calculator sets the DCF field on the click, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=statements-workbench&theme=light&density=compact&focus=1",
  );
  await page.getByRole("tab", { name: "Valuation" }).click();
  const form = page.getByRole("region", { name: "DCF assumptions" });
  const waccField = form.getByRole("textbox", { name: "WACC", exact: true });
  await expect(waccField).toHaveValue("");
  await page.getByText("WACC calculator", { exact: true }).click();
  const calculator = page.getByRole("region", { name: "WACC calculator" });
  const enter = (name: string, value: string) =>
    calculator.getByRole("textbox", { name, exact: true }).fill(value);
  await enter("Equity weight", "0.6");
  await enter("Cost of equity", "0.115");
  await enter("Debt weight", "0.5");
  await enter("Cost of debt", "0.06");
  await enter("Tax rate", "0.25");
  await calculator.getByRole("button", { name: "Calculate" }).click();
  // Rust rejects the weights; no one here sums them.
  await expect(calculator.getByRole("alert")).toContainText("summing to 1.0");
  await enter("Debt weight", "0.4");
  await calculator.getByRole("button", { name: "Calculate" }).click();
  const apply = calculator.getByRole("button", { name: "Use as WACC" });
  await expect(apply).toBeEnabled();
  await expect(waccField).toHaveValue("");
  await apply.click();
  await expect(waccField).toHaveValue(/^0\.08[0-9]*$/);
  expect(errors).toEqual([]);
});
