//! Refs over HTTP (P10, D-046): a ref names the point an application is at,
//! and moving it forward is what makes a step count. A ref moves only from
//! where its mover expects it, so of two movers from one point the second is
//! told where it went (409) and looks again; a follower pulls the steps a ref
//! was moved over after the last point it handled, waiting for the next. A
//! ref is deleted from where its deleter saw it, as it is moved. A ref a
//! package ships is fixed: moving or deleting it is forbidden (403).

use crate::{ApiError, StudioState, engine::blocking};
use axionomy_store::RefPath;
use axionomy_studio_api::{
    RefAdvanceRequest, RefCreateRequest, RefDeleteQuery, RefFrames, RefFramesQuery, RefView,
    advance_ref, create_ref, delete_ref, frames as delivered, read_ref,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use schemars::JsonSchema;
use serde::Deserialize;

/// A ref's name in a URL. A name such as `live/game` is one path segment,
/// its `/` written `%2F`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct RefName {
    pub name: RefPath,
}

pub(crate) async fn create(
    State(state): State<StudioState>,
    Json(request): Json<RefCreateRequest>,
) -> Result<Json<RefView>, ApiError> {
    blocking(&state.engine, move |engine| create_ref(engine, request))
        .await
        .map(Json)
}

pub(crate) async fn read(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
) -> Result<Json<RefView>, ApiError> {
    blocking(&state.engine, move |engine| read_ref(engine, name))
        .await
        .map(Json)
}

pub(crate) async fn advance(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
    Json(request): Json<RefAdvanceRequest>,
) -> Result<Json<RefView>, ApiError> {
    blocking(&state.engine, move |engine| {
        advance_ref(engine, name, request)
    })
    .await
    .map(Json)
}

pub(crate) async fn delete(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
    Query(query): Query<RefDeleteQuery>,
) -> Result<Json<RefView>, ApiError> {
    blocking(&state.engine, move |engine| delete_ref(engine, name, query))
        .await
        .map(Json)
}

pub(crate) async fn frames(
    State(state): State<StudioState>,
    Path(RefName { name }): Path<RefName>,
    Query(query): Query<RefFramesQuery>,
) -> Result<Json<RefFrames>, ApiError> {
    let wait = query.wait().map_err(ApiError)?;
    blocking(&state.engine, move |engine| {
        engine.wait(&name, query.after.point(), wait)?;
        delivered(engine, name, query)
    })
    .await
    .map(Json)
}
