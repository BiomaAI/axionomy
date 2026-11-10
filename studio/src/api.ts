import createClient from "openapi-fetch";
import type { paths, components } from "./generated/api";

export const api = createClient<paths>({ baseUrl: window.location.origin });

export type ProblemDescriptor = components["schemas"]["ProblemDescriptor"];
export type Study = components["schemas"]["Study"];
export type StudyRequest = components["schemas"]["StudyRequest"];
export type StudySummary = components["schemas"]["StudySummary"];
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
export type SearchObservation = components["schemas"]["SearchObservationView"];
export type LeaderboardView = components["schemas"]["LeaderboardView"];
export type LeaderboardEntry = components["schemas"]["LeaderboardEntryView"];

export async function fetchProblems(): Promise<ProblemDescriptor[]> {
  const { data, error } = await api.GET("/api/problems");
  if (error || !data) throw new Error(readError(error));
  return data.problems;
}

export async function fetchStaticProblems(): Promise<ProblemDescriptor[]> {
  const response = await fetch(staticUrl("studies/catalog.json"), { cache: "no-store" });
  if (!response.ok) throw new Error("static problem catalog was unavailable");
  return (await response.json()) as ProblemDescriptor[];
}

export async function createStudy(request: StudyRequest): Promise<StudySummary> {
  const { data, error } = await api.POST("/api/studies", { body: request });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function cancelStudy(studyId: string): Promise<StudySummary> {
  const { data, error } = await api.DELETE("/api/studies/{study_id}", {
    params: { path: { study_id: studyId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function pauseStudy(studyId: string): Promise<StudySummary> {
  const { data, error } = await api.POST("/api/studies/{study_id}/pause", {
    params: { path: { study_id: studyId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function resumeStudy(studyId: string): Promise<StudySummary> {
  const { data, error } = await api.POST("/api/studies/{study_id}/resume", {
    params: { path: { study_id: studyId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function fetchStudy(studyId: string): Promise<Study> {
  const { data, error } = await api.GET("/api/studies/{study_id}/study", {
    params: { path: { study_id: studyId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function fetchStaticStudy(problem: string): Promise<Study> {
  const response = await fetch(staticUrl(`studies/${encodeURIComponent(problem)}.json`));
  if (!response.ok) throw new Error(`static study for ${problem} could not be loaded`);
  return (await response.json()) as Study;
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
