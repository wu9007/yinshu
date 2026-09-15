//! 离线打包本地日志与检测结果，不写入配置密钥。

use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
};

use async_trait::async_trait;
use serde_json::{json, Value};
use yinshu_core::{
    config::{cli_log_dir, default_diagnose_output_path},
    diagnostics::collect_diagnostic_files,
};
use zip::{write::SimpleFileOptions, ZipWriter};

use crate::{Command, CommandError, CommandErrorKind, CommandExecutor, CommandResult};

/// 只执行离线诊断导出的命令执行器。
pub struct DiagnosticsCommandExecutor;

#[async_trait]
impl CommandExecutor for DiagnosticsCommandExecutor {
    async fn execute(&self, command: Command) -> Result<CommandResult, CommandError> {
        match command {
            Command::ExportDiagnostics { path, doctor_json } => {
                let path = write_diagnostics_bundle(path, doctor_json.as_deref())?;
                Ok(CommandResult::Diagnostics { path })
            }
            _ => Err(CommandError::new(
                CommandErrorKind::Unsupported,
                "diagnostics executor only supports export_diagnostics",
            )),
        }
    }
}

/// 创建只处理诊断导出的命令服务，不依赖运行中的 Agent。
pub fn diagnose_command_service() -> std::sync::Arc<crate::CommandService> {
    let executor: std::sync::Arc<dyn CommandExecutor> =
        std::sync::Arc::new(DiagnosticsCommandExecutor);
    std::sync::Arc::new(crate::CommandService::new(None, executor))
}

/// 把日志目录中的可收集文件打成 zip。
pub fn write_diagnostics_bundle(
    output: Option<PathBuf>,
    doctor_json: Option<&str>,
) -> Result<PathBuf, CommandError> {
    let output = match output {
        Some(path) => path,
        None => default_diagnose_output_path().map_err(runtime_io)?,
    };
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(runtime_io)?;
    }

    let log_dir = cli_log_dir().map_err(runtime_io)?;
    let files = collect_diagnostic_files(&log_dir).map_err(runtime_io)?;
    let mut archive_names: Vec<String> = files
        .iter()
        .filter_map(|path| path.file_name()?.to_str().map(ToOwned::to_owned))
        .collect();
    archive_names.sort();
    archive_names.insert(0, "summary.json".into());
    if doctor_json.is_some() {
        archive_names.push("doctor.json".into());
    }

    let summary = json!({
        "product": "yinshu",
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "log_dir": log_dir.display().to_string(),
        "files": archive_names,
    });

    let file = File::create(&output).map_err(runtime_io)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    write_zip_string(&mut zip, "summary.json", &pretty_json(&summary)?, options)?;
    for path in files {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                CommandError::new(CommandErrorKind::Runtime, "diagnostic file name is invalid")
            })?;
        let bytes = fs::read(&path).map_err(runtime_io)?;
        zip.start_file(name, options).map_err(runtime_zip)?;
        zip.write_all(&bytes).map_err(runtime_io)?;
    }
    if let Some(doctor_json) = doctor_json {
        write_zip_string(&mut zip, "doctor.json", doctor_json, options)?;
    }
    zip.finish().map_err(runtime_zip)?;
    Ok(output)
}

fn write_zip_string(
    zip: &mut ZipWriter<File>,
    name: &str,
    contents: &str,
    options: SimpleFileOptions,
) -> Result<(), CommandError> {
    zip.start_file(name, options).map_err(runtime_zip)?;
    zip.write_all(contents.as_bytes()).map_err(runtime_io)
}

fn pretty_json(value: &Value) -> Result<String, CommandError> {
    serde_json::to_string_pretty(value)
        .map_err(|error| CommandError::new(CommandErrorKind::Runtime, error.to_string()))
}

fn runtime_io(error: io::Error) -> CommandError {
    CommandError::new(CommandErrorKind::Runtime, error.to_string())
}

fn runtime_zip(error: zip::result::ZipError) -> CommandError {
    CommandError::new(CommandErrorKind::Runtime, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsString,
        io::Read,
        path::Path,
        sync::{Mutex, OnceLock},
    };
    use yinshu_core::config::LOG_DIR_OVERRIDE_ENV;
    use zip::ZipArchive;

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    fn set_env(key: &'static str, value: &Path) -> EnvGuard {
        let previous = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value);
        }
        EnvGuard { key, previous }
    }

    #[test]
    fn bundle_includes_logs_and_skips_config_secrets() {
        let _lock = test_lock();
        let root = std::env::temp_dir().join(format!(
            "yinshu-cli-diag-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let log_dir = root.join("logs");
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(log_dir.join("yinshu-desktop.log"), "desktop-log").unwrap();
        fs::write(log_dir.join("install.log"), "POSTINSTALL").unwrap();
        fs::write(log_dir.join("config.json"), "{\"service\":{}}").unwrap();
        let output = root.join("out.zip");
        let _guard = set_env(LOG_DIR_OVERRIDE_ENV, &log_dir);

        write_diagnostics_bundle(Some(output.clone()), Some(r#"{"summary":{"fail":0}}"#)).unwrap();

        let mut archive = ZipArchive::new(File::open(&output).unwrap()).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"summary.json".into()));
        assert!(names.contains(&"yinshu-desktop.log".into()));
        assert!(names.contains(&"install.log".into()));
        assert!(names.contains(&"doctor.json".into()));
        assert!(!names.contains(&"config.json".into()));

        let mut summary = String::new();
        archive
            .by_name("summary.json")
            .unwrap()
            .read_to_string(&mut summary)
            .unwrap();
        assert!(summary.contains("yinshu-desktop.log"));
        assert!(!summary.contains("secret"));
        let _ = fs::remove_dir_all(&root);
    }
}
