//! The OpenAPI 3.1 contract, generated from the same route table as the router.
//!
//! Every route is declared once as an [`Endpoint`]: its handler and the types it
//! accepts and returns. The router mounts the handlers and the document is
//! derived from the types' Schemars schemas, so neither can drift from the other.

use axum::routing::MethodRouter;
use indexmap::IndexMap;
use schemars::{JsonSchema, Schema, SchemaGenerator, generate::SchemaSettings};
use serde::Serialize;
use serde_json::{Map, Value};

pub(crate) type SchemaFn = fn(&mut SchemaGenerator) -> Schema;

pub(crate) fn schema_of<T: JsonSchema>(generator: &mut SchemaGenerator) -> Schema {
    generator.subschema_for::<T>()
}

fn object_of<T: JsonSchema>(_: &mut SchemaGenerator) -> Schema {
    SchemaSettings::draft07()
        .with(|settings| settings.inline_subschemas = true)
        .into_generator()
        .into_root_schema_for::<T>()
}

#[derive(Clone, Copy)]
enum Location {
    Path,
    Query,
}

#[derive(Clone, Copy)]
enum Body {
    Json(SchemaFn),
    EventStream(SchemaFn),
}

/// What a request carries: a JSON value, or a file's bytes.
#[derive(Clone, Copy)]
enum Request {
    Json(SchemaFn),
    Bytes(&'static str),
}

/// One documented route and the handler that serves it.
pub(crate) struct Endpoint<S> {
    method: &'static str,
    path: &'static str,
    id: &'static str,
    summary: &'static str,
    tag: &'static str,
    parameters: Vec<(Location, SchemaFn)>,
    request: Option<Request>,
    response: Body,
    conflict: Option<(&'static str, SchemaFn)>,
    fallible: bool,
    route: MethodRouter<S>,
}

impl<S> Endpoint<S> {
    pub(crate) fn new<R: JsonSchema>(
        method: &'static str,
        path: &'static str,
        route: MethodRouter<S>,
    ) -> Self {
        Self {
            method,
            path,
            id: "",
            summary: "",
            tag: "",
            parameters: Vec::new(),
            request: None,
            response: Body::Json(schema_of::<R>),
            conflict: None,
            fallible: false,
            route,
        }
    }

    pub(crate) fn into_route(self) -> (&'static str, MethodRouter<S>) {
        (self.path, self.route)
    }

    pub(crate) fn describe(
        mut self,
        id: &'static str,
        summary: &'static str,
        tag: &'static str,
    ) -> Self {
        self.id = id;
        self.summary = summary;
        self.tag = tag;
        self
    }

    /// Errors are reported as `ErrorResponse` under the default status.
    pub(crate) fn fallible(mut self) -> Self {
        self.fallible = true;
        self
    }

    /// A 409 Conflict, said in `description`, is reported as a `C`.
    pub(crate) fn conflict<C: JsonSchema>(mut self, description: &'static str) -> Self {
        self.conflict = Some((description, schema_of::<C>));
        self
    }

    pub(crate) fn path_parameters<P: JsonSchema>(mut self) -> Self {
        self.parameters.push((Location::Path, object_of::<P>));
        self
    }

    pub(crate) fn query<Q: JsonSchema>(mut self) -> Self {
        self.parameters.push((Location::Query, object_of::<Q>));
        self
    }

    pub(crate) fn json_body<B: JsonSchema>(mut self) -> Self {
        self.request = Some(Request::Json(schema_of::<B>));
        self
    }

    /// The request's body is a file's bytes, said in `description`.
    pub(crate) fn bytes_body(mut self, description: &'static str) -> Self {
        self.request = Some(Request::Bytes(description));
        self
    }

    /// The response is a stream of `E` values in Server-Sent Event data fields.
    pub(crate) fn event_stream<E: JsonSchema>(mut self) -> Self {
        self.response = Body::EventStream(schema_of::<E>);
        self
    }
}

/// A complete OpenAPI 3.1 document. It serializes in declaration order, so
/// the exported contract is stable and reviewable.
#[derive(Debug, Clone, Serialize)]
pub struct OpenApiDocument {
    openapi: &'static str,
    info: Info,
    paths: IndexMap<&'static str, IndexMap<&'static str, Operation>>,
    components: Components,
}

#[derive(Debug, Clone, Serialize)]
struct Info {
    title: &'static str,
    description: &'static str,
    version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Operation {
    tags: [&'static str; 1],
    summary: &'static str,
    operation_id: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parameters: Vec<Parameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_body: Option<RequestBody>,
    responses: IndexMap<&'static str, Response>,
}

#[derive(Debug, Clone, Serialize)]
struct Parameter {
    #[serde(rename = "in")]
    location: &'static str,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    required: Option<bool>,
    schema: Value,
    style: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct RequestBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'static str>,
    content: IndexMap<&'static str, Media>,
    required: bool,
}

#[derive(Debug, Clone, Serialize)]
struct Response {
    description: String,
    content: IndexMap<&'static str, Media>,
}

#[derive(Debug, Clone, Serialize)]
struct Media {
    schema: Value,
}

#[derive(Debug, Clone, Serialize)]
struct Components {
    schemas: Map<String, Value>,
}

fn media(kind: &'static str, schema: Value) -> IndexMap<&'static str, Media> {
    IndexMap::from([(kind, Media { schema })])
}

fn text(description: &str) -> Response {
    Response {
        description: description.into(),
        content: media("text/plain", serde_json::json!({ "type": "string" })),
    }
}

/// The description of the named type the reference points to, as the
/// response description.
fn described(generator: &SchemaGenerator, reference: &Schema) -> String {
    reference
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|target| target.rsplit('/').next())
        .and_then(|name| generator.definitions().get(name))
        .and_then(|definition| definition.get("description"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn parameters(
    generator: &mut SchemaGenerator,
    location: Location,
    object: SchemaFn,
) -> Vec<Parameter> {
    let schema = object(generator);
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    schema
        .get("properties")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .map(|(name, property)| Parameter {
            location: match location {
                Location::Path => "path",
                Location::Query => "query",
            },
            name: name.clone(),
            required: required.contains(&name.as_str()).then_some(true),
            schema: property.clone(),
            style: match location {
                Location::Path => "simple",
                Location::Query => "form",
            },
        })
        .collect()
}

/// The OpenAPI document for `endpoints`, in declaration order, with the
/// schemas of `also`, types the API's clients read that no endpoint serves.
pub(crate) fn document<S>(endpoints: &[Endpoint<S>], also: &[SchemaFn]) -> OpenApiDocument {
    let mut generator = SchemaSettings::draft07()
        .with(|settings| {
            settings.inline_subschemas = false;
            settings.definitions_path = "#/components/schemas/".into();
        })
        .into_generator();
    let mut paths: IndexMap<&'static str, IndexMap<&'static str, Operation>> = IndexMap::new();
    for endpoint in endpoints {
        let parameters = endpoint
            .parameters
            .iter()
            .flat_map(|(location, object)| parameters(&mut generator, *location, *object))
            .collect();
        let request_body = endpoint.request.map(|request| match request {
            Request::Json(schema) => RequestBody {
                description: None,
                content: media("application/json", schema(&mut generator).to_value()),
                required: true,
            },
            Request::Bytes(description) => RequestBody {
                description: Some(description),
                content: media(
                    "application/octet-stream",
                    serde_json::json!({ "type": "string", "contentMediaType": "application/octet-stream" }),
                ),
                required: true,
            },
        });
        let mut responses = IndexMap::new();
        if endpoint.fallible {
            responses.insert(
                "default",
                Response {
                    description: String::new(),
                    content: media(
                        "application/json",
                        schema_of::<crate::ErrorResponse>(&mut generator).to_value(),
                    ),
                },
            );
        }
        let success = match endpoint.response {
            Body::Json(response) => {
                let reference = response(&mut generator);
                Response {
                    description: described(&generator, &reference),
                    content: media("application/json", reference.to_value()),
                }
            }
            Body::EventStream(event) => {
                let reference = event(&mut generator);
                let name = reference
                    .get("$ref")
                    .and_then(Value::as_str)
                    .and_then(|target| target.rsplit('/').next())
                    .unwrap_or("JSON")
                    .to_owned();
                Response {
                    description: format!("Tagged {name} values encoded in SSE data fields"),
                    content: media("text/event-stream", reference.to_value()),
                }
            }
        };
        responses.insert("200", success);
        if let Some((description, conflict)) = endpoint.conflict {
            responses.insert(
                "409",
                Response {
                    description: description.into(),
                    content: media("application/json", conflict(&mut generator).to_value()),
                },
            );
        }
        if let Some(Request::Json(_)) = endpoint.request {
            // Axum's JSON extractor rejects malformed bodies before the handler.
            responses.insert("400", text("Failed to parse the request body as JSON"));
            responses.insert(
                "415",
                text("Expected request with `Content-Type: application/json`"),
            );
            responses.insert(
                "422",
                text("Failed to deserialize the JSON body into the target type"),
            );
        }
        paths.entry(endpoint.path).or_default().insert(
            endpoint.method,
            Operation {
                tags: [endpoint.tag],
                summary: endpoint.summary,
                operation_id: endpoint.id,
                parameters,
                request_body,
                responses,
            },
        );
    }
    for schema in also {
        schema(&mut generator);
    }
    OpenApiDocument {
        openapi: "3.1.0",
        info: Info {
            title: "Axionomy Service API",
            description: "The engine's operations Studio uses: the packages its store holds, loading one, a point's history drawn with its view, what its accounts hold, the candidates at a point, assessing and applying exchanges, strategies and experiments run while their progress streams, comparison, and refs to points, which move forward and deliver the steps they move over; and, until they retire, interface-neutral problem studies.",
            version: env!("CARGO_PKG_VERSION"),
        },
        paths,
        components: Components {
            schemas: generator.take_definitions(true),
        },
    }
}
