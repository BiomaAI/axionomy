import createClient from "openapi-fetch";
import type { paths, components } from "./generated/api";

export const api = createClient<paths>({ baseUrl: window.location.origin });

export type ProblemDescriptor = components["schemas"]["ProblemDescriptor"];
export type RunArtifact = components["schemas"]["RunArtifact"];
export type RunRequest = components["schemas"]["RunRequest"];
export type RunSummary = components["schemas"]["RunSummary"];
export type StudioEvent = components["schemas"]["StudioEvent"];
export type ViewDocument = components["schemas"]["ViewDocument"];
export type ViewSnapshot = components["schemas"]["ViewSnapshot"];
export type ExchangeFrame = components["schemas"]["ExchangeFrame"];
export type Scene = components["schemas"]["Scene"];
export type SceneGlyph = components["schemas"]["SceneGlyphView"];
export type SceneTone = components["schemas"]["SceneToneView"];
export type SceneEntity = components["schemas"]["SceneEntityView"];
export type SceneSurface = components["schemas"]["SceneSurfaceView"];
export type ProposalView = components["schemas"]["ProposalView"];

export async function fetchProblems(): Promise<ProblemDescriptor[]> {
  const { data, error } = await api.GET("/api/problems");
  if (error || !data) throw new Error(readError(error));
  return data.problems;
}

export async function fetchStaticProblems(): Promise<ProblemDescriptor[]> {
  const response = await fetch(staticUrl("artifacts/catalog.json"), { cache: "no-store" });
  if (!response.ok) throw new Error("static problem catalog was unavailable");
  return (await response.json()) as ProblemDescriptor[];
}

export async function createRun(request: RunRequest): Promise<RunSummary> {
  const { data, error } = await api.POST("/api/runs", { body: request });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function cancelRun(runId: string): Promise<RunSummary> {
  const { data, error } = await api.DELETE("/api/runs/{run_id}", {
    params: { path: { run_id: runId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function pauseRun(runId: string): Promise<RunSummary> {
  const { data, error } = await api.POST("/api/runs/{run_id}/pause", {
    params: { path: { run_id: runId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function resumeRun(runId: string): Promise<RunSummary> {
  const { data, error } = await api.POST("/api/runs/{run_id}/resume", {
    params: { path: { run_id: runId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function fetchArtifact(artifactId: string): Promise<RunArtifact> {
  const { data, error } = await api.GET("/api/artifacts/{artifact_id}", {
    params: { path: { artifact_id: artifactId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function fetchStaticArtifact(problem: string): Promise<RunArtifact> {
  const response = await fetch(staticUrl(`artifacts/${encodeURIComponent(problem)}.json`));
  if (!response.ok) throw new Error(`static artifact for ${problem} could not be loaded`);
  return (await response.json()) as RunArtifact;
}

function staticUrl(path: string): URL {
  return new URL(path, new URL(import.meta.env.BASE_URL, window.location.origin));
}

function readError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "error" in error && typeof error.error === "string") {
    return error.error;
  }
  return "Axionomy request failed";
}
