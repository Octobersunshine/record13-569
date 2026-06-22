use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server_port: u16,
    pub upload_dir: PathBuf,
    pub output_dir: PathBuf,
    pub max_upload_size_mb: usize,
    pub default_quality: f32,
    pub allowed_extensions: Vec<String>,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let server_port = std::env::var("SERVER_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(8080);

        let upload_dir = std::env::var("UPLOAD_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./uploads"));

        let output_dir = std::env::var("OUTPUT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("./outputs"));

        let max_upload_size_mb = std::env::var("MAX_UPLOAD_SIZE_MB")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(512);

        let default_quality = std::env::var("DEFAULT_QUALITY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.5);

        let allowed_extensions = std::env::var("ALLOWED_EXTENSIONS")
            .ok()
            .map(|s| s.split(',').map(|x| x.trim().to_lowercase()).collect())
            .unwrap_or_else(|| {
                vec![
                    "obj".into(),
                    "stl".into(),
                ]
            });

        Ok(Self {
            server_port,
            upload_dir,
            output_dir,
            max_upload_size_mb,
            default_quality,
            allowed_extensions,
        })
    }

    pub fn ensure_dirs(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.upload_dir)?;
        std::fs::create_dir_all(&self.output_dir)?;
        Ok(())
    }

    pub fn is_extension_allowed(&self, ext: &str) -> bool {
        let ext = ext.to_lowercase();
        self.allowed_extensions.iter().any(|e| e == &ext)
    }
}
