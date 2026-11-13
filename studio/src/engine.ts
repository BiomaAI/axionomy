/** The engine Studio's packages are served by: the native server, or the engine in the browser's Worker. */
export type EngineKind = "native" | "browser";
export type EngineConnectivity = "checking" | "connected";

/** Whether the native server answers its health check now. */
export async function nativeEngineAnswers(): Promise<boolean> {
  try {
    const response = await fetch(new URL("api/health", window.location.href), {
      cache: "no-store",
      signal: AbortSignal.timeout(1_500),
    });
    if (!response.ok) return false;
    const health = await response.json() as { status?: string; engine?: string };
    return health.status === "ok" && health.engine === "native";
  } catch {
    return false;
  }
}

export function connectionLabel(kind: EngineKind, connectivity: EngineConnectivity): string {
  if (connectivity === "checking") return "Checking for an engine";
  return kind === "browser" ? "Running in your browser" : "Engine connected";
}
