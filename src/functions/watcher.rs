use anyhow::{Context, Result};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};

use crate::functions::{FluxError, storage::FunctionStorage};
use crate::runtime::loader::FunctionLoader;

/// 文件变化事件
#[derive(Debug, Clone)]
pub enum FileChangeEvent {
    /// 文件创建
    Created(PathBuf),
    /// 文件修改
    Modified(PathBuf),
    /// 文件删除
    Deleted(PathBuf),
    /// 文件重命名
    Renamed { from: PathBuf, to: PathBuf },
}

impl FileChangeEvent {
    pub fn path(&self) -> &Path {
        match self {
            FileChangeEvent::Created(path) => path,
            FileChangeEvent::Modified(path) => path,
            FileChangeEvent::Deleted(path) => path,
            FileChangeEvent::Renamed { to, .. } => to,
        }
    }

    pub fn is_rust_file(&self) -> bool {
        self.path().extension().and_then(|s| s.to_str()) == Some("rs")
    }
}

/// 文件监控器配置
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// 监控目录
    pub watch_dirs: Vec<PathBuf>,
    /// 是否递归监控子目录
    pub recursive: bool,
    /// 忽略的文件模式
    pub ignore_patterns: Vec<String>,
    /// 防抖延迟（毫秒）
    pub debounce_ms: u64,
    /// 是否自动重新加载
    pub auto_reload: bool,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            watch_dirs: vec![PathBuf::from("./functions")],
            recursive: true,
            ignore_patterns: vec![
                "*.tmp".to_string(),
                "*.swp".to_string(),
                "*.bak".to_string(),
                ".git".to_string(),
                "target".to_string(),
            ],
            debounce_ms: 500,
            auto_reload: true,
        }
    }
}

/// 函数文件监控器
pub struct FunctionWatcher {
    config: WatcherConfig,
    loader: Arc<FunctionLoader>,
    storage: Arc<dyn FunctionStorage>,
    event_sender: mpsc::UnboundedSender<FileChangeEvent>,
    event_receiver: Arc<RwLock<Option<mpsc::UnboundedReceiver<FileChangeEvent>>>>,
    _watcher: Option<RecommendedWatcher>,
}

impl FunctionWatcher {
    pub fn new(
        config: WatcherConfig,
        loader: Arc<FunctionLoader>,
        storage: Arc<dyn FunctionStorage>,
    ) -> Result<Self> {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Ok(Self {
            config,
            loader,
            storage,
            event_sender,
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            _watcher: None,
        })
    }

    /// 启动文件监控
    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("Starting function file watcher...");

        // 创建文件系统监控器
        let event_sender = self.event_sender.clone();
        let config =
            Config::default().with_poll_interval(Duration::from_millis(self.config.debounce_ms));

        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| match result {
                Ok(event) => {
                    if let Some(change_event) = Self::convert_notify_event(event) {
                        if let Err(e) = event_sender.send(change_event) {
                            tracing::error!("Failed to send file change event: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("File watch error: {}", e);
                }
            },
            config,
        )
        .context("Failed to create file watcher")?;

        // 添加监控目录
        for watch_dir in &self.config.watch_dirs {
            let mode = if self.config.recursive {
                RecursiveMode::Recursive
            } else {
                RecursiveMode::NonRecursive
            };

            if watch_dir.exists() {
                watcher
                    .watch(watch_dir, mode)
                    .with_context(|| format!("Failed to watch directory: {:?}", watch_dir))?;
                tracing::info!("Watching directory: {:?}", watch_dir);
            } else {
                tracing::warn!("Watch directory does not exist: {:?}", watch_dir);
            }
        }

        self._watcher = Some(watcher);

        // 启动事件处理循环
        if self.config.auto_reload {
            self.start_event_processing().await?;
        }

        tracing::info!("Function file watcher started successfully");
        Ok(())
    }

    /// 停止文件监控
    pub async fn stop(&mut self) {
        tracing::info!("Stopping function file watcher...");
        self._watcher = None;
        tracing::info!("Function file watcher stopped");
    }

    /// 启动事件处理循环
    async fn start_event_processing(&self) -> Result<()> {
        let mut receiver = {
            let mut guard = self.event_receiver.write().await;
            guard.take().ok_or_else(|| {
                FluxError::Internal(anyhow::anyhow!("Event receiver already taken"))
            })?
        };

        let loader = self.loader.clone();
        let storage = self.storage.clone();
        let config = self.config.clone();

        tokio::spawn(async move {
            tracing::info!("Started file change event processing loop");

            while let Some(event) = receiver.recv().await {
                if let Err(e) = Self::handle_file_change(&event, &config, &loader, &storage).await {
                    tracing::error!("Failed to handle file change event: {}", e);
                }
            }

            tracing::info!("File change event processing loop ended");
        });

        Ok(())
    }

    /// 处理文件变化事件
    async fn handle_file_change(
        event: &FileChangeEvent,
        config: &WatcherConfig,
        loader: &Arc<FunctionLoader>,
        storage: &Arc<dyn FunctionStorage>,
    ) -> Result<()> {
        // 检查是否是 Rust 文件
        if !event.is_rust_file() {
            return Ok(());
        }

        // 检查是否被忽略
        if Self::should_ignore_file(event.path(), &config.ignore_patterns) {
            return Ok(());
        }

        tracing::debug!("Processing file change event: {:?}", event);

        match event {
            FileChangeEvent::Created(path) | FileChangeEvent::Modified(path) => {
                Self::handle_file_created_or_modified(path, loader, storage).await?;
            }
            FileChangeEvent::Deleted(path) => {
                Self::handle_file_deleted(path, loader, storage).await?;
            }
            FileChangeEvent::Renamed { from, to } => {
                Self::handle_file_renamed(from, to, loader, storage).await?;
            }
        }

        Ok(())
    }

    /// 处理文件创建或修改
    async fn handle_file_created_or_modified(
        path: &Path,
        loader: &Arc<FunctionLoader>,
        storage: &Arc<dyn FunctionStorage>,
    ) -> Result<()> {
        tracing::info!("Reloading function from file: {:?}", path);

        // 从路径提取函数名
        if let Some(name) = Self::extract_function_name_from_path(path) {
            match loader
                .load_function_from_file(path, Some(name.clone()), None, None)
                .await
            {
                Ok(metadata) => {
                    tracing::info!("Successfully reloaded function: {}", name);

                    // 更新存储中的记录
                    if let Ok(Some(mut record)) = storage.load(&name).await {
                        record.metadata = metadata;
                        record.source_path = Some(path.to_path_buf());
                        record.update_code(std::fs::read_to_string(path)?);

                        if let Err(e) = storage.store(&name, record).await {
                            tracing::error!("Failed to update function in storage: {}", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to reload function from {:?}: {}", path, e);
                }
            }
        }

        Ok(())
    }

    /// 处理文件删除
    async fn handle_file_deleted(
        path: &Path,
        _loader: &Arc<FunctionLoader>,
        storage: &Arc<dyn FunctionStorage>,
    ) -> Result<()> {
        if let Some(name) = Self::extract_function_name_from_path(path) {
            tracing::info!("Removing function due to file deletion: {}", name);

            // 从存储中删除
            if let Err(e) = storage.delete(&name).await {
                tracing::error!("Failed to delete function from storage: {}", e);
            } else {
                tracing::info!("Successfully removed function: {}", name);
            }
        }

        Ok(())
    }

    /// 处理文件重命名
    async fn handle_file_renamed(
        from: &Path,
        to: &Path,
        loader: &Arc<FunctionLoader>,
        storage: &Arc<dyn FunctionStorage>,
    ) -> Result<()> {
        // 删除旧函数
        Self::handle_file_deleted(from, loader, storage).await?;

        // 添加新函数
        Self::handle_file_created_or_modified(to, loader, storage).await?;

        Ok(())
    }

    /// 检查文件是否应该被忽略
    fn should_ignore_file(path: &Path, ignore_patterns: &[String]) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in ignore_patterns {
            if let Some(extension) = pattern.strip_prefix("*.") {
                // 处理扩展名模式，如 *.tmp
                if path.extension().and_then(|s| s.to_str()) == Some(extension) {
                    return true;
                }
            } else if path_str.contains(pattern) {
                // 处理其他模式，如 .git
                return true;
            }
        }

        false
    }

    /// 从文件路径提取函数名
    fn extract_function_name_from_path(path: &Path) -> Option<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
    }

    /// 转换 notify 事件为我们的事件类型
    fn convert_notify_event(event: Event) -> Option<FileChangeEvent> {
        match event.kind {
            EventKind::Create(_) => event
                .paths
                .first()
                .map(|path| FileChangeEvent::Created(path.clone())),
            EventKind::Modify(_) => event
                .paths
                .first()
                .map(|path| FileChangeEvent::Modified(path.clone())),
            EventKind::Remove(_) => event
                .paths
                .first()
                .map(|path| FileChangeEvent::Deleted(path.clone())),
            EventKind::Other => {
                // 处理重命名事件
                if event.paths.len() == 2 {
                    Some(FileChangeEvent::Renamed {
                        from: event.paths[0].clone(),
                        to: event.paths[1].clone(),
                    })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 手动触发目录扫描
    pub async fn scan_directories(&self) -> Result<Vec<String>> {
        let mut loaded_functions = Vec::new();

        for watch_dir in &self.config.watch_dirs {
            if watch_dir.exists() {
                match self.loader.load_functions_from_directory(watch_dir).await {
                    Ok(functions) => {
                        let function_names: Vec<String> =
                            functions.iter().map(|f| f.name.clone()).collect();
                        loaded_functions.extend(function_names);
                        tracing::info!(
                            "Scanned directory {:?}, found {} functions",
                            watch_dir,
                            functions.len()
                        );
                    }
                    Err(e) => {
                        tracing::error!("Failed to scan directory {:?}: {}", watch_dir, e);
                    }
                }
            }
        }

        Ok(loaded_functions)
    }

    /// 获取监控状态
    pub fn get_status(&self) -> WatcherStatus {
        WatcherStatus {
            is_active: self._watcher.is_some(),
            watched_directories: self.config.watch_dirs.clone(),
            auto_reload_enabled: self.config.auto_reload,
            debounce_ms: self.config.debounce_ms,
        }
    }
}

/// 监控器状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherStatus {
    pub is_active: bool,
    pub watched_directories: Vec<PathBuf>,
    pub auto_reload_enabled: bool,
    pub debounce_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::functions::storage::MemoryStorage;
    use crate::runtime::loader::FunctionLoader;

    #[tokio::test]
    async fn test_watcher_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WatcherConfig {
            watch_dirs: vec![temp_dir.path().to_path_buf()],
            ..Default::default()
        };

        let storage = Arc::new(MemoryStorage::new());
        let loader = Arc::new(FunctionLoader::new());

        let watcher = FunctionWatcher::new(config, loader, storage);
        assert!(watcher.is_ok());
    }

    #[tokio::test]
    async fn test_file_change_event_conversion() {
        let path = PathBuf::from("test.rs");

        let event = FileChangeEvent::Created(path.clone());
        assert!(event.is_rust_file());
        assert_eq!(event.path(), &path);

        let non_rust_event = FileChangeEvent::Created(PathBuf::from("test.txt"));
        assert!(!non_rust_event.is_rust_file());
    }

    #[tokio::test]
    async fn test_should_ignore_file() {
        let patterns = vec!["*.tmp".to_string(), ".git".to_string()];

        assert!(FunctionWatcher::should_ignore_file(
            Path::new("/path/to/file.tmp"),
            &patterns
        ));

        assert!(FunctionWatcher::should_ignore_file(
            Path::new("/path/.git/config"),
            &patterns
        ));

        assert!(!FunctionWatcher::should_ignore_file(
            Path::new("/path/to/function.rs"),
            &patterns
        ));
    }

    #[tokio::test]
    async fn test_extract_function_name() {
        let path = Path::new("/path/to/my_function.rs");
        let name = FunctionWatcher::extract_function_name_from_path(path);
        assert_eq!(name, Some("my_function".to_string()));

        let path_no_ext = Path::new("/path/to/function");
        let name_no_ext = FunctionWatcher::extract_function_name_from_path(path_no_ext);
        assert_eq!(name_no_ext, Some("function".to_string()));
    }
}
