use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshInfo {
    pub vertex_count: usize,
    pub face_count: usize,
    pub file_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionOptions {
    pub quality: f32,
    pub target_vertex_count: Option<usize>,
    pub target_face_count: Option<usize>,
    pub preserve_borders: bool,
    pub preserve_uvs: bool,
    pub curvature_aware: bool,
    pub curvature_weight: f32,
    pub preserve_features: bool,
    pub feature_threshold: f32,
    pub adaptive_sampling: bool,
    pub min_quality_region: f32,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            quality: 0.5,
            target_vertex_count: None,
            target_face_count: None,
            preserve_borders: true,
            preserve_uvs: true,
            curvature_aware: true,
            curvature_weight: 2.0,
            preserve_features: true,
            feature_threshold: 0.5,
            adaptive_sampling: true,
            min_quality_region: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionTask {
    pub task_id: Uuid,
    pub original_filename: String,
    pub stored_filename: String,
    pub original_path: String,
    pub output_path: Option<String>,
    pub status: TaskStatus,
    pub progress: f32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub original_info: Option<MeshInfo>,
    pub compressed_info: Option<MeshInfo>,
    pub options: CompressionOptions,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub task_id: Uuid,
    pub filename: String,
    pub original_size_bytes: u64,
    pub options: CompressionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatusResponse {
    pub task_id: Uuid,
    pub status: TaskStatus,
    pub progress: f32,
    pub original_filename: String,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub original_info: Option<MeshInfo>,
    pub compressed_info: Option<MeshInfo>,
    pub error_message: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub tasks: Vec<TaskStatusResponse>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}
