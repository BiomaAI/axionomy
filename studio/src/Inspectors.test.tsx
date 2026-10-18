import { render, screen, within } from "@testing-library/react";
import type { RunArtifact } from "./api";
import mission from "../public/artifacts/mission.json";
import { Observations } from "./Inspectors";

test("scrubbing changes private observations and transfers intelligence only after sharing", () => {
  const document = (mission as RunArtifact).documents[0];
  const scan = document.frames.find((frame) => frame.exchange.rate.label.startsWith("Scout discovers Casualty"))!;
  const share = document.frames.find((frame) => frame.exchange.rate.label.startsWith("Brief Medic on Casualty"))!;
  const { rerender } = render(<Observations document={document} frame={scan} />);
  const medic = () => within(screen.getByRole("heading", { name: "Medic" }).closest("article")!);
  expect(medic().queryByText("Known 0, East")).not.toBeInTheDocument();
  rerender(<Observations document={document} frame={share} />);
  expect(medic().getAllByText("Known 0, East").length).toBeGreaterThan(0);
  expect(screen.queryByText("Hidden mission sites and reports")).not.toBeInTheDocument();
  rerender(<Observations document={document} />);
  expect(screen.queryByText("Known 0, East")).not.toBeInTheDocument();
});
