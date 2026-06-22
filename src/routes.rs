use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Multipart, Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::StatusCode;
use axum::response::{AppendHeaders, IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::model::{HealthResponse, PresetInfo, TaskListResponse, TaskStatusResponse, UploadResponse};
use crate::task::TaskManager;
use crate::uploader;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub task_manager: Arc<TaskManager>,
    pub start_time: chrono::DateTime<Utc>,
}

pub type SharedState = State<AppState>;

pub fn create_router(config: AppConfig, task_manager: Arc<TaskManager>) -> axum::Router {
    let state = AppState {
        config,
        task_manager,
        start_time: Utc::now(),
    };

    axum::Router::new()
        .route("/health", axum::routing::get(health_handler))
        .route("/api/upload", axum::routing::post(upload_handler))
        .route("/api/tasks", axum::routing::get(list_tasks_handler))
        .route(
            "/api/tasks/:task_id",
            axum::routing::get(task_status_handler),
        )
        .route(
            "/api/tasks/:task_id/download",
            axum::routing::get(download_handler),
        )
        .with_state(state)
}

async fn health_handler(State(state): SharedState) -> Json<HealthResponse> {
    let uptime = Utc::now() - state.start_time;
    Json(HealthResponse {
        status: "ok".to_string(),
        uptime_seconds: uptime.num_seconds() as u64,
        presets: PresetInfo::all(),
    })
}

async fn upload_handler(
    State(state): SharedState,
    multipart: Multipart,
) -> Result<Json<UploadResponse>, AppError> {
    let upload = uploader::handle_upload(&state.config, multipart).await?;

    let task_id = state.task_manager.create_task(&upload).await?;

    let _ = state
        .task_manager
        .analyze_and_fill_original_info(&task_id)
        .await;

    Ok(Json(upload))
}

async fn list_tasks_handler(
    State(state): SharedState,
) -> Result<Json<TaskListResponse>, AppError> {
    let list = state.task_manager.list_tasks().await;
    Ok(Json(list))
}

async fn task_status_handler(
    State(state): SharedState,
    Path(task_id): Path<Uuid>,
) -> Result<Json<TaskStatusResponse>, AppError> {
    let status = state.task_manager.get_task_status(&task_id).await?;
    Ok(Json(status))
}

async fn download_handler(
    State(state): SharedState,
    Path(task_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let task = state.task_manager.get_task(&task_id).await?;

    if task.status != crate::model::TaskStatus::Completed {
        return Err(AppError::BadRequest(format!(
            "Task is not completed yet. Current status: {:?}",
            task.status
        )));
    }

    let output_path = task
        .output_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::Internal("No output path available".into()))?;

    let metadata = tokio::fs::metadata(&output_path).await.map_err(AppError::Io)?;
    if !metadata.is_file() {
        return Err(AppError::Internal("Output file not found".into()));
    }

    let file = tokio::fs::File::open(&output_path).await.map_err(AppError::Io)?;

    let filename = output_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("model.obj")
        .to_string();

    let orig_stem = PathBuf::from(&task.original_filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "model".into());

    let download_filename = format!("{}_compressed.obj", orig_stem);

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let content_type = guess_content_type(&filename);

    let headers = AppendHeaders([
        (
            CONTENT_TYPE,
            content_type.to_string(),
        ),
        (
            CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                download_filename.replace('"', "\\\"")
            ),
        ),
    ]);

    Ok((StatusCode::OK, headers, body).into_response())
}

fn guess_content_type(filename: &str) -> &'static str {
    let ext = std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "obj" => "application/x-tgif",
        "stl" => "model/stl",
        "ply" => "application/octet-stream",
        "fbx" => "application/octet-stream",
        "gltf" => "model/gltf+json",
        "glb" => "model/gltf-binary",
        "dae" => "application/xml",
        "3ds" => "application/x-3ds",
        "x" => "application/octet-stream",
        "blend" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}
