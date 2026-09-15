//! 本地诊断文件筛选与崩溃落盘，不含配置密钥。

use crate::config::{cli_log_dir, INSTALL_LOG_FILE_NAME, STARTUP_ERROR_FILE_NAME};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// 判断日志目录中的文件是否可打进诊断包。
pub fn is_collectable_diagnostic_file(name: &str) -> bool {
    let file_name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let lower = file_name.to_ascii_lowercase();
    if lower == "config.json"
        || lower.ends_with(".sqlite3")
        || lower.ends_with(".sqlite")
        || lower.contains("password")
        || lower.contains("yinshu-config")
    {
        return false;
    }
    is_log_file(&lower)
        || lower == STARTUP_ERROR_FILE_NAME
        || lower == INSTALL_LOG_FILE_NAME
        || (lower.starts_with("crash-") && lower.ends_with(".txt"))
}

fn is_log_file(lower: &str) -> bool {
    lower.ends_with(".log")
        || lower.rsplit_once(".log.").is_some_and(|(_, suffix)| {
            !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit())
        })
}

/// 列出日志目录中可收集的诊断文件。
pub fn collect_diagnostic_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(files),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_collectable_diagnostic_file)
        {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// 按时间戳生成崩溃日志文件名。
pub fn crash_log_file_name(unix_secs: u64) -> String {
    format!("crash-{unix_secs}.txt")
}

/// 把崩溃说明写入日志目录。
pub fn write_crash_log(message: &str) -> io::Result<PathBuf> {
    let dir = cli_log_dir()?;
    fs::create_dir_all(&dir)?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let path = dir.join(crash_log_file_name(secs));
    fs::write(&path, format!("{message}\n"))?;
    Ok(path)
}

/// 安装写入崩溃日志的 panic hook。
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = write_crash_log(&info.to_string());
        previous(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        cli_log_dir, tests::set_env, tests::test_lock, LOG_DIR_OVERRIDE_ENV,
        STARTUP_ERROR_FILE_NAME,
    };
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn collectable_files_keep_logs_and_skip_secrets() {
        assert!(is_collectable_diagnostic_file("yinshu-desktop.log"));
        assert!(is_collectable_diagnostic_file("yinshu-desktop.log.1"));
        assert!(is_collectable_diagnostic_file(INSTALL_LOG_FILE_NAME));
        assert!(is_collectable_diagnostic_file(STARTUP_ERROR_FILE_NAME));
        assert!(is_collectable_diagnostic_file("crash-1710000000.txt"));
        assert!(!is_collectable_diagnostic_file("config.json"));
        assert!(!is_collectable_diagnostic_file(
            "yinshu-config-encrypted.json"
        ));
        assert!(!is_collectable_diagnostic_file("task_history.sqlite3"));
        assert!(!is_collectable_diagnostic_file("password.txt"));
    }

    #[test]
    fn collect_diagnostic_files_skips_config_and_keeps_logs() {
        let dir = std::env::temp_dir().join(format!("yinshu-diag-files-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("yinshu-desktop.log"), "log").unwrap();
        fs::write(dir.join("config.json"), "{\"secret\":true}").unwrap();
        fs::write(dir.join("install.log"), "PREINSTALL").unwrap();

        let names: Vec<_> = collect_diagnostic_files(&dir)
            .unwrap()
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, vec!["install.log", "yinshu-desktop.log"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_crash_log_uses_the_log_dir() {
        let _lock = test_lock();
        let log_dir = std::env::temp_dir().join(format!("yinshu-crash-{}", Uuid::new_v4()));
        let _guard = set_env(LOG_DIR_OVERRIDE_ENV, &log_dir);

        let path = write_crash_log("boom").unwrap();
        assert_eq!(path.parent().unwrap(), cli_log_dir().unwrap());
        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("crash-") && name.ends_with(".txt")));
        assert_eq!(fs::read_to_string(&path).unwrap(), "boom\n");
        let _ = fs::remove_dir_all(&log_dir);
    }
}
