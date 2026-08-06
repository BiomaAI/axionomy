import createClient from "openapi-fetch";
import type { paths, components } from "./generated/api";

export const api = createClient<paths>({ baseUrl: window.location.origin });

export type ExampleSummary = components["schemas"]["ExampleSummary"];
export type RunSummary = components["schemas"]["RunSummary"];
export type StudioEvent = components["schemas"]["StudioEvent"];
export type ViewDocument = components["schemas"]["ViewDocument"];
export type ViewSnapshot = components["schemas"]["ViewSnapshot"];
export type ExchangeFrame = components["schemas"]["ExchangeFrame"];
export type Scene = components["schemas"]["Scene"];

export async function fetchExamples(): Promise<ExampleSummary[]> {
  const { data } = await api.GET("/api/examples");
  if (!data) throw new Error("example catalog was unavailable");
  return data.examples;
}

export async function createRun(example: string): Promise<RunSummary> {
  const { data, error } = await api.POST("/api/runs", {
    body: { example },
  });
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

export async function fetchDocument(documentId: string): Promise<ViewDocument> {
  const { data, error } = await api.GET("/api/traces/{document_id}", {
    params: { path: { document_id: documentId } },
  });
  if (error || !data) throw new Error(readError(error));
  return data;
}

export async function fetchStaticDocument(): Promise<ViewDocument> {
  const response = await fetch("/examples/maze-pareto-energy.json");
  if (!response.ok) throw new Error("static example could not be loaded");
  return (await response.json()) as ViewDocument;
}

function readError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "error" in error && typeof error.error === "string") {
    return error.error;
  }
  return "Axionomy Studio request failed";
}
