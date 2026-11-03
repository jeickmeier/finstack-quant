"use client";
import { useState } from "react";
import {
  PortfolioReturns,
  type PortfolioReturnsInputs,
} from "@/components/finstack/portfolio/components/portfolio-returns/portfolio-returns";
import {
  useMwrXirr,
  useTwrrLinked,
  useTwrrModifiedDietz,
} from "@/hooks/portfolio/use-portfolio-returns/use-portfolio-returns";

interface ReturnsPanelProps {
  /** The host's returns, period and flows; each form starts from them and is computed when the tab opens. */
  inputs: PortfolioReturnsInputs;
  density: "compact" | "comfortable";
}

/**
 * The book's performance from host-supplied data: one native twrrLinked,
 * twrrModifiedDietz and mwrXirr per submitted form, mounted (and so called)
 * when the Returns tab first opens.
 */
export function ReturnsPanel({ inputs, density }: ReturnsPanelProps) {
  const [linkedRequest, setLinkedRequest] = useState(inputs.linked);
  const [period, setPeriod] = useState(inputs.dietz);
  const [cashflows, setCashflows] = useState(inputs.xirr);
  const linked = useTwrrLinked(linkedRequest);
  const dietz = useTwrrModifiedDietz(period);
  const xirr = useMwrXirr(cashflows);
  return (
    <div className="flex flex-col gap-4">
      <p className="text-xs text-muted-foreground">
        Rust computes each return from the host&apos;s performance data, not
        from the valued book.
      </p>
      <PortfolioReturns
        defaultInputs={inputs}
        onSubmitLinked={setLinkedRequest}
        onSubmitDietz={setPeriod}
        onSubmitXirr={setCashflows}
        linked={{
          value: linked.data ?? null,
          loading: linked.isFetching,
          error: linked.error?.message,
        }}
        dietz={{
          value: dietz.data ?? null,
          loading: dietz.isFetching,
          error: dietz.error?.message,
        }}
        xirr={{
          value: xirr.data ?? null,
          loading: xirr.isFetching,
          error: xirr.error?.message,
        }}
        density={density}
      />
    </div>
  );
}
