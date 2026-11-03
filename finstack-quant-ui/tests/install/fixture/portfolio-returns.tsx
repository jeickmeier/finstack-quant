"use client";
import {
  PortfolioReturns,
  type PortfolioReturnsInputs,
} from "@/components/finstack/portfolio/components/portfolio-returns/portfolio-returns";

import data from "./data.json";
// The returns are the supplied native fixture: the item installs without a worker.
const { inputs, cases } = data.portfolioReturns;
export function InstalledItem() {
  return (
    <>
      <p>Supplied valid fixture; standalone portfolio returns.</p>
      <PortfolioReturns
        defaultInputs={inputs as PortfolioReturnsInputs}
        onSubmitLinked={() => {}}
        onSubmitDietz={() => {}}
        onSubmitXirr={() => {}}
        linked={{ value: cases.linked.annual.result }}
        dietz={{ value: cases.dietz.contribution.result }}
        xirr={{ value: cases.xirr.conventional.result }}
      />
    </>
  );
}
