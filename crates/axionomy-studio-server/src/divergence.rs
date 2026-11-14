//! Two points' histories compared over HTTP: where they part and how they
//! differ after, as `axionomy-studio-api` defines it.

use crate::{ApiError, StudioState, engine::blocking};
use axionomy_studio_api::{DivergenceQuery, DivergenceView};
use axum::{
    Json,
    extract::{Query, State},
};

pub(crate) async fn divergence(
    State(state): State<StudioState>,
    Query(query): Query<DivergenceQuery>,
) -> Result<Json<DivergenceView>, ApiError> {
    blocking(&state.engine, move |engine| {
        axionomy_studio_api::divergence(engine, query)
    })
    .await
    .map(Json)
}
