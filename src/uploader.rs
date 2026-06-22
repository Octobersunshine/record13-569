use std::path::{Path, PathBuf};

use axum::extract::Multipart;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::model::{CompressionOptions, QualityPreset, UploadResponse};

pub struct UploadedFile {
    pub task_id: Uuid,
    pub original_filename: String,
    pub stored_filename: String,
    pub stored_path: PathBuf,
    pub file_size: u64,
    pub options: CompressionOptions,
}

fn get_extension(filename: &str) -> Option<String> {
    Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
}

struct PendingFields {
    preset: Option<String>,
    quality: Option<String>,
    target_vertex_count: Option<String>,
    target_face_count: Option<String>,
    preserve_borders: Option<String>,
    preserve_uvs: Option<String>,
    curvature_aware: Option<String>,
    curvature_weight: Option<String>,
    preserve_features: Option<String>,
    feature_threshold: Option<String>,
    adaptive_sampling: Option<String>,
    min_quality_region: Option<String>,
}

impl PendingFields {
    fn new() -> Self {
        Self {
            preset: None,
            quality: None,
            target_vertex_count: None,
            target_face_count: None,
            preserve_borders: None,
            preserve_uvs: None,
            curvature_aware: None,
            curvature_weight: None,
            preserve_features: None,
            feature_threshold: None,
            adaptive_sampling: None,
            min_quality_region: None,
        }
    }
}

fn apply_pending_fields(options: &mut CompressionOptions, pending: PendingFields) {
    if let Some(preset_str) = pending.preset {
        if let Some(preset) = QualityPreset::from_str(&preset_str) {
            options.apply_preset(preset);
        }
    }

    if let Some(v) = pending.quality {
        if let Ok(q) = v.parse::<f32>() {
            options.quality = q.clamp(0.01, 1.0);
        }
    }
    if let Some(v) = pending.target_vertex_count {
        if let Ok(n) = v.parse::<usize>() {
            options.target_vertex_count = Some(n);
        }
    }
    if let Some(v) = pending.target_face_count {
        if let Ok(n) = v.parse::<usize>() {
            options.target_face_count = Some(n);
        }
    }
    if let Some(v) = pending.preserve_borders {
        options.preserve_borders = v != "false" && v != "0";
    }
    if let Some(v) = pending.preserve_uvs {
        options.preserve_uvs = v != "false" && v != "0";
    }
    if let Some(v) = pending.curvature_aware {
        options.curvature_aware = v != "false" && v != "0";
    }
    if let Some(v) = pending.curvature_weight {
        if let Ok(n) = v.parse::<f32>() {
            options.curvature_weight = n.clamp(0.0, 10.0);
        }
    }
    if let Some(v) = pending.preserve_features {
        options.preserve_features = v != "false" && v != "0";
    }
    if let Some(v) = pending.feature_threshold {
        if let Ok(n) = v.parse::<f32>() {
            options.feature_threshold = n.clamp(0.0, 1.0);
        }
    }
    if let Some(v) = pending.adaptive_sampling {
        options.adaptive_sampling = v != "false" && v != "0";
    }
    if let Some(v) = pending.min_quality_region {
        if let Ok(n) = v.parse::<f32>() {
            options.min_quality_region = n.clamp(0.0, 1.0);
        }
    }
}

pub async fn handle_upload(
    config: &AppConfig,
    mut multipart: Multipart,
) -> Result<UploadResponse, AppError> {
    let mut uploaded_file: Option<UploadedFile> = None;
    let mut pending = PendingFields::new();
    let mut options: CompressionOptions = CompressionOptions::default();
    let task_id = Uuid::new_v4();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::InvalidMultipart(e.to_string()))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                let original_filename = field
                    .file_name()
                    .ok_or_else(|| AppError::InvalidMultipart("Missing filename".into()))?
                    .to_string();

                let ext = get_extension(&original_filename).ok_or_else(|| {
                    AppError::UnsupportedFormat(format!(
                        "Unable to determine extension from filename: {}",
                        original_filename
                    ))
                })?;

                if !config.is_extension_allowed(&ext) {
                    return Err(AppError::UnsupportedFormat(format!(
                        "Extension '{}' is not allowed. Allowed: {:?}",
                        ext, config.allowed_extensions
                    )));
                }

                let stored_filename = format!("{}.{}", task_id, ext);
                let stored_path = config.upload_dir.join(&stored_filename);
                let max_bytes = config.max_upload_size_mb * 1024 * 1024;

                let mut file = tokio::fs::File::create(&stored_path)
                    .await
                    .map_err(AppError::Io)?;

                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::InvalidMultipart(e.to_string()))?;

                let total_bytes = data.len() as u64;

                if total_bytes > max_bytes as u64 {
                    let _ = tokio::fs::remove_file(&stored_path).await;
                    return Err(AppError::FileTooLarge(format!(
                        "File size {} bytes exceeds limit of {} bytes",
                        total_bytes, max_bytes
                    )));
                }

                file.write_all(&data).await.map_err(AppError::Io)?;
                file.flush().await.map_err(AppError::Io)?;
                drop(file);

                uploaded_file = Some(UploadedFile {
                    task_id,
                    original_filename,
                    stored_filename,
                    stored_path,
                    file_size: total_bytes,
                    options: options.clone(),
                });
            }
            "preset" => {
                pending.preset = Some(field.text().await.unwrap_or_default());
            }
            "quality" => {
                pending.quality = Some(field.text().await.unwrap_or_default());
            }
            "target_vertex_count" => {
                pending.target_vertex_count = Some(field.text().await.unwrap_or_default());
            }
            "target_face_count" => {
                pending.target_face_count = Some(field.text().await.unwrap_or_default());
            }
            "preserve_borders" => {
                pending.preserve_borders = Some(field.text().await.unwrap_or_default());
            }
            "preserve_uvs" => {
                pending.preserve_uvs = Some(field.text().await.unwrap_or_default());
            }
            "curvature_aware" => {
                pending.curvature_aware = Some(field.text().await.unwrap_or_default());
            }
            "curvature_weight" => {
                pending.curvature_weight = Some(field.text().await.unwrap_or_default());
            }
            "preserve_features" => {
                pending.preserve_features = Some(field.text().await.unwrap_or_default());
            }
            "feature_threshold" => {
                pending.feature_threshold = Some(field.text().await.unwrap_or_default());
            }
            "adaptive_sampling" => {
                pending.adaptive_sampling = Some(field.text().await.unwrap_or_default());
            }
            "min_quality_region" => {
                pending.min_quality_region = Some(field.text().await.unwrap_or_default());
            }
            _ => {
                let _ = field.text().await;
            }
        }
    }

    apply_pending_fields(&mut options, pending);

    let mut uploaded = uploaded_file.ok_or_else(|| {
        AppError::InvalidMultipart("No file field found in multipart request".into())
    })?;
    uploaded.options = options.clone();

    Ok(UploadResponse {
        task_id: uploaded.task_id,
        filename: uploaded.original_filename,
        original_size_bytes: uploaded.file_size,
        options,
    })
}
