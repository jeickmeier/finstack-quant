"use client";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";
import {
  PricingParamsForm,
  type PricingParams,
} from "@/components/finstack/valuations/components/pricing-params-form/pricing-params-form";

/** Settings chrome. Theme and density still stamp the document so portaled menus match. */
export function WorkbenchSettings({
  params,
  onParamsChange,
  instrumentType,
  models,
  metrics,
  loading,
  error,
  density,
  onDensityChange,
  theme,
  onThemeChange,
}: {
  params: PricingParams;
  onParamsChange(value: PricingParams): void;
  instrumentType: string;
  models: Readonly<Record<string, readonly string[]>>;
  metrics: Readonly<Record<string, readonly string[]>>;
  loading: boolean;
  error?: string;
  density: "compact" | "comfortable";
  onDensityChange(value: "compact" | "comfortable"): void;
  theme: "light" | "dark" | undefined;
  onThemeChange(value: "light" | "dark"): void;
}) {
  return (
    <Popover>
      <PopoverTrigger render={<Button variant="outline" size="sm" />}>
        Settings
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-[28rem] max-w-[calc(100vw-2rem)]"
      >
        <div className="max-h-[60vh] space-y-4 overflow-auto">
          <PricingParamsForm
            value={params}
            onValueChange={onParamsChange}
            instrumentType={instrumentType}
            models={models}
            metrics={metrics}
            loading={loading}
            error={error}
            sections={["metrics", "advanced"]}
          />
          <Label className="flex items-center justify-between gap-3">
            Density
            <Select
              items={[
                { value: "compact", label: "Compact" },
                { value: "comfortable", label: "Comfortable" },
              ]}
              value={density}
              onValueChange={(value) => {
                if (value !== "compact" && value !== "comfortable") return;
                document.documentElement.dataset.density = value;
                onDensityChange(value);
              }}
            >
              <SelectTrigger aria-label="Workbench density">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="compact">Compact</SelectItem>
                <SelectItem value="comfortable">Comfortable</SelectItem>
              </SelectContent>
            </Select>
          </Label>
          <Button
            variant="outline"
            size="sm"
            type="button"
            onClick={(event) => {
              const inherited = event.currentTarget
                .closest("[data-theme]")
                ?.getAttribute("data-theme");
              const nextTheme =
                (theme ?? inherited) === "dark" ? "light" : "dark";
              document.documentElement.dataset.theme = nextTheme;
              document.documentElement.classList.toggle(
                "dark",
                nextTheme === "dark",
              );
              onThemeChange(nextTheme);
            }}
          >
            Toggle theme
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
