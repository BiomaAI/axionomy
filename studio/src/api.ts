import createClient from "openapi-fetch";
import type { paths, components } from "./generated/api";

export const api = createClient<paths>({ baseUrl: window.location.origin });

export type ViewDocument = components["schemas"]["ViewDocument"];
export type ViewSnapshot = components["schemas"]["ViewSnapshot"];
export type ExchangeFrame = components["schemas"]["ExchangeFrame"];
export type Scene = components["schemas"]["Scene"];
export type SceneGlyph = components["schemas"]["SceneGlyphView"];
export type SceneEntity = components["schemas"]["SceneEntityView"];
export type SceneSurface = components["schemas"]["SceneSurfaceView"];
