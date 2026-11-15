import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import type { RateData } from "./api";
import { ExchangeEditor, instanceOf, pathUnder, variableChoices, writeExchange, type Draft } from "./ExchangeEditor";
import type { Candidate, Exchange } from "./packageEngine";

const role = (path: string, label: string) => ({ role: { key: `paths/role/${path}`, label }, consumed: [], produced: [], preserved: [] });

/** `step/{from}/{to}`: from and to as a relation allows, the walker fixed, a witness free, and a reach read by a condition. */
const step: RateData = {
  rate: { key: "paths/rate/step/from/to", label: "step/{from}/{to}" },
  variables: [
    { name: "from", domain: { kind: "range", first: 1, last: 3 } },
    { name: "to", domain: { kind: "range", first: 1, last: 3 } },
  ],
  relations: [{ variables: ["from", "to"], rows: [["1", "2"], ["2", "3"]] }],
  accounts: [{ role: { key: "paths/role/walker", label: "Walker" }, account: { key: "paths/account/walker", label: "Walker" } }],
  instances: 2,
  roles: [role("walker", "Walker"), role("witness", "Witness")],
  distinct_roles: [],
  parameters: ["reach"],
};

const accounts = [{ key: "paths/account/walker", label: "Walker" }, { key: "paths/account/bystander", label: "Bystander" }];

describe("writing an exchange from a rate", () => {
  test("a pattern writes the identifier its variables' values give", () => {
    expect(instanceOf("step/{from}/{to}", { from: "1", to: "2" })).toBe("step/1/2");
    expect(instanceOf("stabilize/{patient}", { patient: "0" })).toBe("stabilize/0");
    expect(instanceOf("step/{from}/{to}", { from: "1" })).toBeUndefined();
    expect(instanceOf("finish", {})).toBe("finish");
  });

  test("a view's key names its path under the ontology and category", () => {
    expect(pathUnder("rescue/incident", "rate", "rescue/incident/rate/close")).toBe("close");
    expect(pathUnder("maze", "account", "maze/account/agent")).toBe("agent");
    expect(pathUnder("maze", "rate", "maze/account/agent")).toBeUndefined();
    expect(pathUnder("maze", "rate", "maze/rate/")).toBeUndefined();
  });

  test("a variable offers the values its relations allow with the values before it", () => {
    expect(variableChoices(step, {}, 0)).toEqual(["1", "2"]);
    expect(variableChoices(step, { from: "2" }, 1)).toEqual(["3"]);
    const apart: RateData = { ...step, relations: [], distinct_variables: [["from", "to"]] };
    expect(variableChoices(apart, { from: "2" }, 1)).toEqual(["1", "3"]);
  });

  test("an account the rate fixes through a pattern is written with the variables' values", () => {
    const moving: RateData = { ...step, accounts: [{ role: { key: "paths/role/walker", label: "Walker" }, account: { key: "paths/account/cell/to", label: "cell/{to}" } }], parameters: [] };
    const draft: Draft = { rate: moving.rate.key, values: { from: "1", to: "2" }, bindings: { "paths/role/witness": "paths/account/bystander" }, units: "1", parameters: {} };
    expect(writeExchange(moving, draft, "paths")).toEqual({ exchange: { rate: "step/1/2", bindings: { walker: "cell/2", witness: "bystander" }, units: "1", parameters: {} } });
    expect(writeExchange(moving, { ...draft, bindings: {} }, "paths")).toEqual({ lacks: "an account for Witness" });
    expect(writeExchange(moving, { ...draft, units: "0" }, "paths")).toEqual({ lacks: "units, a whole number from 1" });
  });

  test("the editor writes the instance, checks it, and applies it only once it applies", async () => {
    const checked: Exchange[] = [];
    const applied = vi.fn(async () => undefined);
    const onCheck = vi.fn(async (exchange: Exchange): Promise<Candidate> => {
      checked.push(exchange);
      return {
        exchange,
        proposal: {
          id: "exchange/0",
          label: "Step from 2 to 3",
          description: "",
          snapshot_index: 0,
          exchange: { rate: { key: "paths/rate/step/2/3", label: "Step from 2 to 3" }, law: step.rate, arguments: [{ variable: "from", value: "2" }, { variable: "to", value: "3" }], units: "1", bindings: [] },
          assessment: { status: "applicable", accounts: [], projected_deltas: [], shortfalls: [], issues: [] },
        },
      };
    });
    const { container } = render(<ExchangeEditor rates={[step]} accounts={accounts} ontology="paths" point="run-0:0" onCheck={onCheck} onApply={applied} />);
    const apply = screen.getByRole("button", { name: "Apply" });
    // The walker is fixed; the witness is chosen.
    expect(container.querySelector("output")).toHaveTextContent("Walker");
    expect(screen.getByRole("button", { name: "Check" })).toBeDisabled();
    fireEvent.change(container.querySelector("select[name='variable-from']")!, { target: { value: "2" } });
    expect(container.querySelector<HTMLSelectElement>("select[name='variable-to']")!.value).toBe("3");
    expect(container.querySelector(".written")).toHaveTextContent("step/2/3");
    fireEvent.change(container.querySelector("select[name='role-paths/role/witness']")!, { target: { value: "paths/account/bystander" } });
    fireEvent.change(container.querySelector("input[name='parameter-reach']")!, { target: { value: "5" } });
    expect(apply).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Check" }));
    await waitFor(() => expect(apply).toBeEnabled());
    expect(checked).toEqual([{ rate: "step/2/3", bindings: { walker: "walker", witness: "bystander" }, units: "1", parameters: { reach: "5" } }]);
    // Changed after the check, it is not applied until checked again.
    fireEvent.change(container.querySelector("input[name='exchange-units']")!, { target: { value: "2" } });
    expect(apply).toBeDisabled();
    fireEvent.change(container.querySelector("input[name='exchange-units']")!, { target: { value: "1" } });
    fireEvent.click(apply);
    await waitFor(() => expect(applied).toHaveBeenCalledWith(checked[0], "Step from 2 to 3"));
  });
});
