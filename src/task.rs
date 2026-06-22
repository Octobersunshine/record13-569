use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::compressor::{CompressionResult, Compressor};
use crate::config::AppConfig;
use crate::error::AppError;
use crate::model::{
    CompressionTask, MeshInfo, TaskListResponse, TaskStatus, TaskStatusResponse, UploadResponse,
};

pub struct TaskManager {
    config: AppConfig,
    tasks: RwLock<HashMap<Uuid, CompressionTask>>,
    compressor: Compressor,
    _queue: Mutex<()>,
}

impl TaskManager {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            tasks: RwLock::new(HashMap::new()),
            compressor: Compressor::new(),
            _queue: Mutex::new(()),
        }
    }

    pub async fn create_task(
        self: &Arc<Self>,
        upload: &UploadResponse,
    ) -> Result<Uuid, AppError> {
        let now = Utc::now();
        let task_id = upload.task_id;

        let ext = get_extension(&upload.filename).unwrap_or_else(|| "obj".to_string());
        let stored_filename = format!("{}.{}", task_id, ext);
        let input_path = self.config.upload_dir.join(&stored_filename);
        let output_name = format!("{}_compressed.obj", task_id);
        let output_path = self.config.output_dir.join(output_name);

        let task = CompressionTask {
            task_id,
            original_filename: upload.filename.clone(),
            stored_filename,
            original_path: input_path.to_string_lossy().to_string(),
            output_path: Some(output_path.to_string_lossy().to_string()),
            status: TaskStatus::Queued,
            progress: 0.0,
            created_at: now,
            started_at: None,
            completed_at: None,
            original_info: None,
            compressed_info: None,
            options: upload.options.clone(),
            error_message: None,
        };

        self.tasks.write().await.insert(task_id, task);

        let manager_clone = self.clone();
        tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async move {
                let _ = manager_clone.execute_task(task_id).await;
            });
        });

        Ok(task_id)
    }

    async fn execute_task(self: &Arc<Self>, task_id: Uuid) -> Result<(), AppError> {
        tracing::info!("Starting task: {}", task_id);

        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = TaskStatus::Processing;
                task.started_at = Some(Utc::now());
                task.progress = 0.05;
            }
        }

        let (input_path, output_path_opt, options) = {
            let tasks = self.tasks.read().await;
            let task = tasks
                .get(&task_id)
                .ok_or_else(|| AppError::TaskNotFound(task_id.to_string()))?;
            let input = PathBuf::from(&task.original_path);
            let output = task.output_path.as_ref().map(PathBuf::from);
            let opts = task.options.clone();
            (input, output, opts)
        };

        let output_path =
            output_path_opt.ok_or_else(|| AppError::Internal("No output path set".into()))?;

        let self_clone = self.clone();
        let result = tokio::task::spawn_blocking(move || {
            let compressor = self_clone.compressor.clone();
            let task_id_c = task_id;
            let manager = self_clone;

            let update_progress = move |p: f32| {
                let rt = tokio::runtime::Handle::current();
                let mgr = manager.clone();
                let tid = task_id_c;
                rt.block_on(async move {
                    let mut tasks = mgr.tasks.write().await;
                    if let Some(t) = tasks.get_mut(&tid) {
                        t.progress = p;
                    }
                });
            };

            compressor.compress(&input_path, &output_path, &options, update_progress)
        })
        .await
        .map_err(|e| AppError::Internal(format!("Task join error: {}", e)))?;

        match result {
            Ok(CompressionResult {
                output_path: _,
                original_info,
                compressed_info,
                curvature_stats: _,
            }) => {
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(&task_id) {
                    task.status = TaskStatus::Completed;
                    task.completed_at = Some(Utc::now());
                    task.progress = 1.0;
                    task.original_info = Some(original_info);
                    task.compressed_info = Some(compressed_info);
                }
                tracing::info!("Task completed: {}", task_id);
            }
            Err(e) => {
                tracing::error!("Task failed: {}, error: {}", task_id, e);
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(&task_id) {
                    task.status = TaskStatus::Failed;
                    task.completed_at = Some(Utc::now());
                    task.error_message = Some(e.to_string());
                }
            }
        }

        Ok(())
    }

    pub async fn get_task(&self, task_id: &Uuid) -> Result<CompressionTask, AppError> {
        let tasks = self.tasks.read().await;
        tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| AppError::TaskNotFound(task_id.to_string()))
    }

    pub async fn get_task_status(&self, task_id: &Uuid) -> Result<TaskStatusResponse, AppError> {
        let task = self.get_task(task_id).await?;
        Ok(task_to_status_response(&task))
    }

    pub async fn list_tasks(&self) -> TaskListResponse {
        let tasks = self.tasks.read().await;
        let mut list: Vec<TaskStatusResponse> = tasks
            .values()
            .map(task_to_status_response)
            .collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let total = list.len();
        TaskListResponse { tasks: list, total }
    }

    pub async fn analyze_and_fill_original_info(
        &self,
        task_id: &Uuid,
    ) -> Result<Option<MeshInfo>, AppError> {
        let input_path = {
            let tasks = self.tasks.read().await;
            let task = tasks
                .get(task_id)
                .ok_or_else(|| AppError::TaskNotFound(task_id.to_string()))?;
            PathBuf::from(&task.original_path)
        };

        let compressor = self.compressor.clone();
        let info =
            tokio::task::spawn_blocking(move || compressor.analyze_mesh(&input_path)).await;

        match info {
            Ok(Ok(mesh_info)) => {
                let mut tasks = self.tasks.write().await;
                if let Some(task) = tasks.get_mut(task_id) {
                    task.original_info = Some(mesh_info.clone());
                }
                Ok(Some(mesh_info))
            }
            Ok(Err(e)) => {
                tracing::warn!("Failed to analyze original mesh: {}", e);
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("Join error analyzing mesh: {}", e);
                Ok(None)
            }
        }
    }
}

fn get_extension(filename: &str) -> Option<String> {
    std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
}

fn task_to_status_response(task: &CompressionTask) -> TaskStatusResponse {
    let download_url = if task.status == TaskStatus::Completed {
        task.output_path
            .as_ref()
            .map(|_| format!("/api/tasks/{}/download", task.task_id))
    } else {
        None
    };

    TaskStatusResponse {
        task_id: task.task_id,
        status: task.status.clone(),
        progress: task.progress,
        original_filename: task.original_filename.clone(),
        created_at: task.created_at,
        started_at: task.started_at,
        completed_at: task.completed_at,
        original_info: task.original_info.clone(),
        compressed_info: task.compressed_info.clone(),
        error_message: task.error_message.clone(),
        download_url,
    }
}

impl Default for TaskManager {
    fn default() -> Self {
        Self::new(AppConfig::from_env().unwrap_or_else(|_| {
            AppConfig {
                server_port: 8080,
                upload_dir: PathBuf::from("./uploads"),
                output_dir: PathBuf::from("./outputs"),
                max_upload_size_mb: 512,
                default_quality: 0.5,
                allowed_extensions: vec!["obj".into(), "stl".into(), "fbx".into()],
            }
        }))
    }
}
