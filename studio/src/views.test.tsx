import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { AccountData, ExchangeData } from "./api";
import { ExchangeView } from "./ExchangeView";
import { IdLink, InspectionProvider, InspectorDock, type Subject } from "./Inspectors";
import { AccountView, StateList, ledgerRows } from "./Ledger";
import { ResultsTable } from "./PackageViews";
import type { ResultsView } from "./packageEngine";
import { RateTable, RateView } from "./RateView";

const asset = (path: string) => ({ key: `paths/asset/${path}`, label: path });
const account = (path: string, balances: Record<string, string>): AccountData => ({
  account: { key: `paths/account/${path}`, label: path },
  balances: Object.entries(balances).map(([held, quantity]) => ({ asset: asset(held), quantity })),
});

describe("AccountView", () => {
  const before = account("walker", { coin: "1", badge: "2", key: "4" });
  const after = account("walker", { coin: "3", badge: "2", map: "1" });

  test("reads each balance against before: changed, new, gone, or the same", () => {
    expect(ledgerRows(after, before).map((row) => [row.asset.label, row.before, row.now])).toEqual([
      ["coin", "1", "3"],
      ["badge", undefined, "2"],
      ["map", "0", "1"],
      ["key", "4", undefined],
    ]);
  });

  test("shows before → after, and folds the balances the timeline never changes", () => {
    const changing = new Set(["paths/asset/coin", "paths/asset/map", "paths/asset/key"]);
    const { container } = render(<AccountView account={after} before={before} changing={changing} />);
    const rows = [...container.querySelectorAll(".ledger > .balance")].map((row) => row.textContent);
    expect(rows).toEqual(["coin1 →3", "map0 →1", "key4 →0"]);
    expect(container.querySelector(".balance.ghost")).toHaveTextContent("key");
    expect(screen.getByText("1 unchanged")).toBeInTheDocument();
    expect(container.querySelector(".unchanged-assets .balance")).toHaveTextContent("badge2");
  });

  test("folds nothing where nothing of it changes", () => {
    const { container } = render(<AccountView account={after} changing={new Set()} />);
    expect(container.querySelectorAll(".ledger > .balance")).toHaveLength(3);
    expect(container.querySelector(".unchanged-assets")).toBeNull();
  });

  test("the state list shows what the step changed of each account, and folds the rest", () => {
    const { container } = render(<StateList accounts={[after, account("world", { gate: "1" })]} before={[before, account("world", { gate: "1" })]} />);
    expect([...container.querySelectorAll(".state > .state-list > li")].map((item) => item.textContent)).toEqual(["walker3 changedcoin1 →3map0 →1key4 →0"]);
    expect(container.querySelector(".unchanged-accounts summary")).toHaveTextContent("1 unchanged");
    expect(container.querySelector(".unchanged-accounts li")).toHaveTextContent("world");
  });

  test("at the start, the state list is every account", () => {
    render(<StateList accounts={[after, account("world", { gate: "1" })]} />);
    expect(screen.getAllByRole("listitem").map((item) => item.textContent)).toEqual(["walker", "world"]);
  });
});

describe("ExchangeView", () => {
  const exchange: ExchangeData = {
    rate: { key: "paths/rate/step/1/2", label: "Step from 1 to 2" },
    law: { key: "paths/rate/step/from/to", label: "step/{from}/{to}" },
    arguments: [{ variable: "from", value: "1" }, { variable: "to", value: "2" }],
    units: "2",
    bindings: [{ role: { key: "paths/role/walker", label: "Walker" }, account: { key: "paths/account/walker", label: "walker" } }],
    parameters: [{ name: "reach", value: "5" }],
  };
  const assessment = {
    status: "applicable" as const,
    accounts: [],
    shortfalls: [],
    issues: [],
    projected_deltas: [{ account: exchange.bindings[0].account, consumed: [{ asset: asset("at/1"), quantity: "1" }], produced: [{ asset: asset("at/2"), quantity: "1" }], preserved: [] }],
  };

  test("compact: the rate as written with its values, the bindings, units and parameters, and whether it applies", () => {
    const { container } = render(<ExchangeView exchange={exchange} assessment={assessment} />);
    expect(container.querySelector(".exchange-rate")).toHaveTextContent("Step from 1 to 2");
    expect(container.querySelector(".written")).toHaveTextContent("step/1/2");
    expect(container.querySelector(".written b")).toHaveAttribute("title", "from");
    expect(container.querySelector(".exchange-bindings")).toHaveTextContent("Walker walker");
    expect(container.querySelector(".exchange-bindings")).toHaveTextContent("reach 5");
    expect(container.querySelector(".exchange-units")).toHaveTextContent("×2");
    expect(container.querySelector(".exchange-status")).toHaveTextContent("applies");
    expect(container.querySelector(".effects")).toBeNull();
  });

  test("full: what each account gives, keeps and gets, and the assessment", () => {
    const { container } = render(<ExchangeView form="full" exchange={exchange} assessment={assessment} />);
    expect(container.querySelector(".effect-gives")).toHaveTextContent("at/1");
    expect(container.querySelector(".effect-gets")).toHaveTextContent("at/2");
    expect(container.querySelector(".effect-keeps")).toBeNull();
    expect(container.querySelector(".assessment-applicable")).toHaveTextContent("applicable");
  });
});

describe("RateView", () => {
  const rate = {
    rate: { key: "paths/rate/step/from/to", label: "step/{from}/{to}" },
    variables: [{ name: "from", domain: { kind: "range" as const, first: 1, last: 3 } }, { name: "kind", domain: { kind: "values" as const, values: ["walk", "run"] } }],
    distinct_variables: [["from", "kind"]],
    relations: [{ variables: ["from", "kind"], gives: ["pace"], rows: [["1", "walk", "4"]] }],
    derived: [{ name: "cost", value: "({from} − 1)" }],
    accounts: [{ role: { key: "paths/role/walker", label: "Walker" }, account: { key: "paths/account/walker", label: "walker" } }],
    instances: 1,
    roles: [{ role: { key: "paths/role/walker", label: "Walker" }, consumed: [{ asset: asset("at/from"), quantity: "1" }], produced: [], preserved: [] }],
    distinct_roles: [],
    conditions: [{ name: "near", left: "{from}", comparison: "≤", right: "$reach" }],
    parameters: ["reach"],
  };

  test("shows variables, domains, relations and what their rows give, derived values, fixed accounts, baskets and conditions", () => {
    const { container } = render(<RateView rate={rate} />);
    expect(container).toHaveTextContent("{from}1..3");
    expect(container).toHaveTextContent("{kind}walk, run");
    expect(container).toHaveTextContent("1 instance");
    expect(container).toHaveTextContent("{from} ≠ {kind}");
    expect(container.querySelector("table")).toHaveTextContent("{from}{kind}{pace}1walk4");
    expect(container.querySelector("th.given")).toHaveTextContent("{pace}");
    expect(container).toHaveTextContent("{cost}({from} − 1)");
    expect(container.querySelector(".rate-role-name")).toHaveTextContent("Walker walker");
    expect(container.querySelector(".effect-gives")).toHaveTextContent("at/from");
    expect(container).toHaveTextContent("near{from} ≤ $reach");
  });

  test("the rate table lists rates as written and filters them", () => {
    const { container } = render(<RateTable rates={[rate, { ...rate, rate: { key: "paths/rate/finish", label: "finish" }, variables: [], instances: undefined }]} />);
    expect(container.querySelectorAll("tbody tr")).toHaveLength(2);
    fireEvent.change(screen.getByRole("searchbox", { name: "Filter rates" }), { target: { value: "fin" } });
    expect([...container.querySelectorAll("tbody tr")].map((row) => row.textContent)).toEqual(["finish1Walker"]);
  });
});

describe("ResultsTable", () => {
  test("lists each arm's trials with how they ended and their objectives", () => {
    const results: ResultsView = {
      experiment: "maze/micro",
      objectives: [{ objective: "maze/energy", direction: "minimize" }],
      arms: [{ arm: { strategy: "maze/a-star" }, start: "run-0:0", trials: [{ end: "run-1:3", ended: { kind: "reached" }, seed: "17", values: ["6"] }] }],
    };
    const watch = vi.fn();
    const { container } = render(<ResultsTable results={results} onWatch={watch} />);
    expect(container.querySelector("thead")).toHaveTextContent("energy ↓");
    expect(container.querySelector("tbody tr")).toHaveTextContent("a-star1reached the goal36");
    fireEvent.click(screen.getByRole("button", { name: "Watch" }));
    expect(watch).toHaveBeenCalledWith({ end: "run-1:3", start: "run-0:0" });
  });
});

describe("IdLink", () => {
  const subject: Subject = { kind: "account", id: { key: "paths/account/walker", label: "Walker" } };

  test("without inspection, is its label with its path", () => {
    render(<IdLink subject={subject} />);
    expect(screen.getByText("Walker")).toHaveAttribute("title", "paths/account/walker");
  });

  test("previews a card on hover, pins on click, and a pinned inspector closes", () => {
    vi.useFakeTimers();
    try {
      const pin = vi.fn();
      const inspect = (inspected: Subject) => <p>inspecting {inspected.kind}</p>;
      render(<InspectionProvider value={{ inspect, pin }}><IdLink subject={subject} /></InspectionProvider>);
      fireEvent.mouseEnter(screen.getByRole("button", { name: "Walker" }));
      act(() => { vi.advanceTimersByTime(300); });
      expect(screen.getByRole("tooltip")).toHaveTextContent("AccountWalkerpaths/account/walkerinspecting account");
      fireEvent.click(screen.getByRole("button", { name: "Walker" }));
      expect(pin).toHaveBeenCalledWith(subject);
      expect(screen.queryByRole("tooltip")).toBeNull();
    } finally {
      vi.useRealTimers();
    }
    const close = vi.fn();
    render(<InspectorDock pinned={[subject, { kind: "step", step: 3 }]} inspect={(inspected) => <p>{inspected.kind}</p>} onClose={close} />);
    fireEvent.click(screen.getByRole("button", { name: "Close Walker" }));
    expect(close).toHaveBeenLastCalledWith(subject);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(close).toHaveBeenLastCalledWith({ kind: "step", step: 3 });
  });
});
