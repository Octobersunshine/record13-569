use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum QualityPreset {
    Hd,
    Standard,
    Minimal,
}

impl QualityPreset {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "hd" | "high" | "高清" => Some(QualityPreset::Hd),
            "standard" | "std" | "default" | "标准" => Some(QualityPreset::Standard),
            "minimal" | "min" | "极简" | "low" => Some(QualityPreset::Minimal),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            QualityPreset::Hd => "hd",
            QualityPreset::Standard => "standard",
            QualityPreset::Minimal => "minimal",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            QualityPreset::Hd => "高清",
            QualityPreset::Standard => "标准",
            QualityPreset::Minimal => "极简",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            QualityPreset::Hd => "保留大部分细节，适合近景展示和高精度渲染",
            QualityPreset::Standard => "在质量与体积间取得平衡，通用场景推荐",
            QualityPreset::Minimal => "极致压缩，适合Web加载和低性能设备预览",
        }
    }

    pub fn all() -> Vec<QualityPreset> {
        vec![QualityPreset::Hd, QualityPreset::Standard, QualityPreset::Minimal]
    }

    pub fn to_options(&self) -> CompressionOptions {
        match self {
            QualityPreset::Hd => CompressionOptions {
                quality: 0.85,
                target_vertex_count: None,
                target_face_count: None,
                preserve_borders: true,
                preserve_uvs: true,
                curvature_aware: true,
                curvature_weight: 3.0,
                preserve_features: true,
                feature_threshold: 0.35,
                adaptive_sampling: true,
                min_quality_region: 0.92,
                preset: Some(QualityPreset::Hd),
            },
            QualityPreset::Standard => CompressionOptions {
                quality: 0.55,
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
                preset: Some(QualityPreset::Standard),
            },
            QualityPreset::Minimal => CompressionOptions {
                quality: 0.2,
                target_vertex_count: None,
                target_face_count: None,
                preserve_borders: false,
                preserve_uvs: true,
                curvature_aware: true,
                curvature_weight: 1.2,
                preserve_features: false,
                feature_threshold: 0.8,
                adaptive_sampling: false,
                min_quality_region: 0.5,
                preset: Some(QualityPreset::Minimal),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub default_quality: f32,
    pub curvature_aware: bool,
    pub preserve_features: bool,
}

impl PresetInfo {
    pub fn all() -> Vec<PresetInfo> {
        QualityPreset::all()
            .iter()
            .map(|p| {
                let opts = p.to_options();
                PresetInfo {
                    key: p.as_str().to_string(),
                    name: p.display_name().to_string(),
                    description: p.description().to_string(),
                    default_quality: opts.quality,
                    curvature_aware: opts.curvature_aware,
                    preserve_features: opts.preserve_features,
                }
            })
            .collect()
    }
}

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
    #[serde(default)]
    pub preset: Option<QualityPreset>,
}

impl Default for CompressionOptions {
    fn default() -> Self {
        Self {
            quality: 0.55,
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
            preset: Some(QualityPreset::Standard),
        }
    }
}

impl CompressionOptions {
    pub fn from_preset(preset: QualityPreset) -> Self {
        preset.to_options()
    }

    pub fn apply_preset(&mut self, preset: QualityPreset) {
        let preset_opts = preset.to_options();
        self.quality = preset_opts.quality;
        self.preserve_borders = preset_opts.preserve_borders;
        self.preserve_uvs = preset_opts.preserve_uvs;
        self.curvature_aware = preset_opts.curvature_aware;
        self.curvature_weight = preset_opts.curvature_weight;
        self.preserve_features = preset_opts.preserve_features;
        self.feature_threshold = preset_opts.feature_threshold;
        self.adaptive_sampling = preset_opts.adaptive_sampling;
        self.min_quality_region = preset_opts.min_quality_region;
        self.preset = Some(preset);
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
    pub presets: Vec<PresetInfo>,
}
