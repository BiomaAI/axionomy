import createClient from "openapi-fetch";
import type { paths, components } from "./generated/api";

export const api = createClient<paths>({ baseUrl: window.location.origin });

type Schemas = components["schemas"];

export type ViewDocument = Schemas["ViewDocument"];
export type ViewSnapshot = Schemas["ViewSnapshot"];
export type ExchangeFrame = Schemas["ExchangeFrame"];
export type Scene = Schemas["Scene"];
export type SceneGlyph = Schemas["SceneGlyphView"];
export type SceneEntity = Schemas["SceneEntityView"];
export type SceneSurface = Schemas["SceneSurfaceView"];

/** A value of the ontology as a view shows it: `key`, a path, joins; `label` is for people. */
export type ViewId = Schemas["ViewId"];
/** An account and what it holds. */
export type AccountData = Schemas["AccountView"];
/** An asset and how much of it, exactly. */
export type QuantityData = Schemas["AssetQuantityView"];
/** An exchange as a view shows it: its rate, the values it gives the rate's variables, its bindings, units and parameters. */
export type ExchangeData = Schemas["ExchangeView"];
export type AssessmentData = Schemas["AssessmentView"];
/** What one account gave, kept and got. */
export type DeltaData = Schemas["AccountDeltaView"];
/** A rate as written: its variables, relations, roles, baskets and conditions. */
export type RateData = Schemas["RateView"];
export type ModelData = Schemas["ModelView"];
