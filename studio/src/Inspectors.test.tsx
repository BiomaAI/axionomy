import { render, screen, within } from "@testing-library/react";
import type { RunArtifact } from "./api";
import mission from "../public/artifacts/mission.json";
import { Observations } from "./Inspectors";

test("scrubbing changes private observations and transfers intelligence only after sharing", () => {
  const document = (mission as RunArtifact).documents[0];
  const scan = document.frames.find((frame) => frame.exchange.rate.label === "Scout sees South")!;
  const share = document.frames.find((frame) => frame.exchange.rate.label === "Scout shares sighting: South")!;
  const { rerender } = render(<Observations document={document} frame={scan} />);
  const medic = () => within(screen.getByRole("heading", { name: "Medic" }).closest("article")!);
  expect(medic().queryByText("Private sighting: South")).not.toBeInTheDocument();
  rerender(<Observations document={document} frame={share} />);
  expect(medic().getAllByText("Private sighting: South").length).toBeGreaterThan(0);
  expect(screen.queryByText("Objective at North")).not.toBeInTheDocument();
  rerender(<Observations document={document} />);
  expect(screen.queryByText("Private sighting: South")).not.toBeInTheDocument();
});
