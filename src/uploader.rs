use std::path::{Path, PathBuf};

use axum::extract::Multipart;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::model::{CompressionOptions, UploadResponse};

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

pub async fn handle_upload(
    config: &AppConfig,
    mut multipart: Multipart,
) -> Result<UploadResponse, AppError> {
    let mut uploaded_file: Option<UploadedFile> = None;
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

                let mut total_bytes: u64 = 0;
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::InvalidMultipart(e.to_string()))?;

                total_bytes += data.len() as u64;

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
            "quality" => {
                let value = field.text().await.unwrap_or_default();
                if let Ok(q) = value.parse::<f32>() {
                    options.quality = q.clamp(0.01, 1.0);
                }
            }
            "target_vertex_count" => {
                let value = field.text().await.unwrap_or_default();
                if let Ok(v) = value.parse::<usize>() {
                    options.target_vertex_count = Some(v);
                }
            }
            "target_face_count" => {
                let value = field.text().await.unwrap_or_default();
                if let Ok(v) = value.parse::<usize>() {
                    options.target_face_count = Some(v);
                }
            }
            "preserve_borders" => {
                let value = field.text().await.unwrap_or_default();
                options.preserve_borders = value != "false" && value != "0";
            }
            "preserve_uvs" => {
                let value = field.text().await.unwrap_or_default();
                options.preserve_uvs = value != "false" && value != "0";
            }
            _ => {
                let _ = field.text().await;
            }
        }
    }

    let uploaded = uploaded_file.ok_or_else(|| {
        AppError::InvalidMultipart("No file field found in multipart request".into())
    })?;

    Ok(UploadResponse {
        task_id: uploaded.task_id,
        filename: uploaded.original_filename,
        original_size_bytes: uploaded.file_size,
        options,
    })
}
