use crate::{
    config::AgentConfig,
    document::{test_page_to_pdf, DocumentError, TestPageContent, TestPageRow, TestPageSection},
    printing::{
        paper_name, PaperInfo, PrintError, PrintOptions, PrinterAvailability, PrinterInfo,
        PrinterTrayInfo,
    },
    protocol::EffectivePaper,
    state::AgentState,
    task_history::{NewTaskHistoryEvent, TaskHistorySource, TaskHistoryStatus},
};
use std::{
    net::UdpSocket,
    path::{Path, PathBuf},
    process::Command,
};
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

/// 测试打印时可能返回的错误。
#[derive(Debug, Error)]
pub enum TestPrintError {
    #[error("printer not configured")]
    PrinterNotConfigured,
    #[error("paper not configured")]
    PaperNotConfigured,
    #[error("document generation failed: {0}")]
    Document(#[from] DocumentError),
    #[error("print failed: {0}")]
    Print(#[from] PrintError),
}

/// 测试页上展示的本机信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub hostname: String,
    pub username: String,
    pub os: String,
    pub local_ip: Option<String>,
    pub printed_at: String,
}

impl DeviceInfo {
    /// 采集当前机器上测试页常用的本机信息。
    pub fn current() -> Self {
        Self {
            hostname: current_hostname(),
            username: current_username(),
            os: current_os_label(),
            local_ip: local_lan_ip(),
            printed_at: current_printed_at(),
        }
    }
}

/// 使用当前默认打印设置生成并提交一张配置测试页。
pub async fn print_test_page(state: &AgentState) -> Result<(), TestPrintError> {
    let config = state.config.read().await.clone();
    print_test_page_with_config(state, config).await
}

/// 使用指定配置生成并提交一张配置测试页，不持久化配置。
pub async fn print_test_page_with_config(
    state: &AgentState,
    config: AgentConfig,
) -> Result<(), TestPrintError> {
    let printer_name = config
        .printing
        .default_printer
        .clone()
        .ok_or(TestPrintError::PrinterNotConfigured)?;
    let paper = config
        .printing
        .default_paper
        .clone()
        .ok_or(TestPrintError::PaperNotConfigured)?;
    let printer = state
        .printing
        .list_printers()
        .ok()
        .and_then(|printers| printers.into_iter().find(|item| item.name == printer_name));
    let path = test_page_temp_path();
    let paper_label = paper_name(paper.width_mm, paper.height_mm);
    let job_id = uuid::Uuid::new_v4().to_string();
    let result = print_test_page_inner(
        state,
        &config,
        &printer_name,
        paper,
        printer.as_ref(),
        &path,
    )
    .await;
    let _ = std::fs::remove_file(&path);
    if !matches!(
        result,
        Err(TestPrintError::Print(PrintError::PrinterOffline))
    ) {
        record_test_print_history(
            state,
            &job_id,
            &printer_name,
            &paper_label,
            result.as_ref().err().map(ToString::to_string).as_deref(),
        );
    }

    result
}

fn record_test_print_history(
    state: &AgentState,
    job_id: &str,
    printer_name: &str,
    paper_name: &str,
    error: Option<&str>,
) {
    let Some(task_history) = &state.task_history else {
        return;
    };
    let occurred_at = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string());
    let (status, message) = match error {
        Some(message) => (TaskHistoryStatus::Failed, Some(message)),
        None => (
            TaskHistoryStatus::Submitted,
            Some("submitted to system print queue"),
        ),
    };
    if let Err(error) = task_history.record_event(&NewTaskHistoryEvent {
        job_id,
        request_id: None,
        batch_id: None,
        source: TaskHistorySource::Test,
        status,
        message,
        printer_name: Some(printer_name),
        paper_name: Some(paper_name),
        copies: Some(1),
        occurred_at: &occurred_at,
    }) {
        log::error!("failed to record test print history: {error}");
    }
}

async fn print_test_page_inner(
    state: &AgentState,
    config: &AgentConfig,
    printer_name: &str,
    paper: EffectivePaper,
    printer: Option<&PrinterInfo>,
    path: &Path,
) -> Result<(), TestPrintError> {
    let trays = state
        .printing
        .list_trays(printer_name)
        .unwrap_or_default();
    let content = build_test_page(
        config,
        printer_name,
        &paper,
        &DeviceInfo::current(),
        printer,
        &trays,
    );
    test_page_to_pdf(&paper, &content, path)?;

    let options = PrintOptions {
        printer_name: printer_name.to_string(),
        paper: paper_info_from_effective(&paper),
        copies: 1,
    };
    let _print_guard = state.print_lock.lock().await;
    state.printing.print_pdf(path, &options)?;

    Ok(())
}

fn build_test_page(
    config: &AgentConfig,
    printer_name: &str,
    paper: &EffectivePaper,
    device: &DeviceInfo,
    printer: Option<&PrinterInfo>,
    trays: &[PrinterTrayInfo],
) -> TestPageContent {
    let connect = match device.local_ip.as_deref() {
        Some(ip) => format!("{ip}:{}", config.service.port),
        None => format!("port {}", config.service.port),
    };

    let mut printer_rows = vec![
        row("Name", printer_name),
        row("Status", &printer_status_line(printer)),
    ];
    if let Some(detail) = printer_detail_line(printer) {
        printer_rows.push(row("Via", &detail));
    }
    if printer.is_some_and(|item| item.is_default) {
        printer_rows.push(row("Default", "Yes"));
    }

    let tray_rows = if trays.is_empty() {
        vec![row("", "Not reported")]
    } else {
        trays
            .iter()
            .take(8)
            .map(|tray| row("", &tray_label(tray)))
            .chain((trays.len() > 8).then(|| row("", &format!("+{} more", trays.len() - 8))))
            .collect()
    };

    let paper_label = paper_name(paper.width_mm, paper.height_mm);
    let paper_rows = if paper_label.ends_with("mm") {
        vec![row("", &paper_label)]
    } else {
        vec![
            row("", &paper_label),
            row(
                "",
                &format!(
                    "{} x {} mm",
                    format_paper_dimension(paper.width_mm),
                    format_paper_dimension(paper.height_mm)
                ),
            ),
        ]
    };

    TestPageContent {
        title: "Yinshu".into(),
        kicker: "Test page".into(),
        subtitle: "This paper reached this printer.".into(),
        sections: vec![
            TestPageSection {
                heading: "Printer".into(),
                rows: printer_rows,
            },
            TestPageSection {
                heading: "Trays".into(),
                rows: tray_rows,
            },
            TestPageSection {
                heading: "Paper".into(),
                rows: paper_rows,
            },
            TestPageSection {
                heading: "Computer".into(),
                rows: vec![
                    row("Host", empty_as_dash(&device.hostname)),
                    row("User", empty_as_dash(&device.username)),
                    row("OS", empty_as_dash(&device.os)),
                    row("Web", &connect),
                    row("Sites", &join_or_none(&config.security.allowed_origins)),
                ],
            },
        ],
        footer: device.printed_at.clone(),
    }
}

fn row(label: &str, value: &str) -> TestPageRow {
    TestPageRow {
        label: label.to_string(),
        value: value.to_string(),
    }
}

fn printer_status_line(printer: Option<&PrinterInfo>) -> String {
    match printer.map(|item| item.availability) {
        Some(PrinterAvailability::Available) => "Ready".into(),
        Some(PrinterAvailability::Unavailable) => "Offline".into(),
        Some(PrinterAvailability::Unknown) | None => "Not reported".into(),
    }
}

fn printer_detail_line(printer: Option<&PrinterInfo>) -> Option<String> {
    let printer = printer?;
    let mut parts = Vec::new();
    if let Some(kind) = printer_kind_label(printer) {
        parts.push(kind.to_string());
    }
    if let Some(port) = printer.port.as_deref().filter(|value| !value.is_empty()) {
        parts.push(port.to_string());
    }
    if let Some(dpi) = printer.dpi {
        parts.push(format!("{dpi} dpi"));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("  /  "))
    }
}

fn tray_label(tray: &PrinterTrayInfo) -> String {
    let name = tray.name.replace('_', " ");
    if name == tray.id {
        name
    } else {
        format!("{name}  {}", tray.id)
    }
}

fn printer_kind_label(printer: &PrinterInfo) -> Option<&'static str> {
    if printer.is_virtual == Some(true) {
        return Some("Virtual");
    }
    if printer.is_network == Some(true) {
        return Some("Network");
    }
    if printer.is_local == Some(true) {
        return Some("Local");
    }
    None
}

fn join_or_none(values: &[String]) -> String {
    if values.is_empty() {
        return "None".into();
    }
    values.join(", ")
}

fn empty_as_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "-"
    } else {
        value
    }
}

fn current_hostname() -> String {
    if let Ok(name) = std::env::var("COMPUTERNAME") {
        if !name.trim().is_empty() {
            return name;
        }
    }
    if let Ok(output) = Command::new("hostname").output() {
        if let Ok(name) = String::from_utf8(output.stdout) {
            let name = name.trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into())
}

fn current_username() -> String {
    ["USER", "USERNAME", "LOGNAME"]
        .into_iter()
        .find_map(|key| {
            std::env::var(key)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "unknown".into())
}

fn current_os_label() -> String {
    let os = match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    format!("{os} {}", std::env::consts::ARCH)
}

/// 探测本机常用局域网地址，与测试页 LAN 行相同。
pub fn local_lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    Some(socket.local_addr().ok()?.ip().to_string())
}

fn current_printed_at() -> String {
    let stamp = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "unknown".into());
    let stamp = stamp.get(..16).unwrap_or(stamp.as_str()).replace('T', " ");
    format!("{stamp} UTC")
}

fn test_page_temp_path() -> PathBuf {
    std::env::temp_dir().join(format!("yinshu-test-page-{}.pdf", uuid::Uuid::new_v4()))
}

fn paper_info_from_effective(paper: &EffectivePaper) -> PaperInfo {
    PaperInfo {
        id: format!(
            "custom_{}x{}mm",
            format_paper_dimension(paper.width_mm),
            format_paper_dimension(paper.height_mm)
        ),
        name: paper_name(paper.width_mm, paper.height_mm),
        width_mm: paper.width_mm,
        height_mm: paper.height_mm,
    }
}

fn format_paper_dimension(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{build_test_page, print_test_page_with_config, DeviceInfo};
    use crate::{
        config::AgentConfig,
        document::TestPageContent,
        printing::{
            PaperInfo, PrintBackend, PrintError, PrintOptions, PrintResult, PrintSubmission,
            PrinterInfo, PrinterTrayInfo, RawPrintOptions,
        },
        protocol::EffectivePaper,
        state::AgentState,
        task_history::{TaskHistorySource, TaskHistoryStatus, TaskHistoryStore},
    };
    use std::path::Path;

    fn visible_text(content: &TestPageContent) -> Vec<String> {
        let mut lines = vec![
            content.title.clone(),
            content.kicker.clone(),
            content.subtitle.clone(),
        ];
        for section in &content.sections {
            lines.push(section.heading.clone());
            for row in &section.rows {
                lines.push(format!("{} {}", row.label, row.value));
            }
        }
        lines.push(content.footer.clone());
        lines
    }

    fn sample_device() -> DeviceInfo {
        DeviceInfo {
            hostname: "studio.local".into(),
            username: "ethan".into(),
            os: "macOS aarch64".into(),
            local_ip: Some("192.168.1.23".into()),
            printed_at: "2026-09-15 00:37 UTC".into(),
        }
    }

    #[test]
    fn test_page_lists_printer_trays_and_status() {
        let mut config = AgentConfig::default();
        config.service.port = 17890;
        config.security.allowed_origins = vec!["https://erp.example.com".into()];
        let paper = EffectivePaper {
            width_mm: 60.0,
            height_mm: 40.0,
        };
        let printer = PrinterInfo {
            name: "Zebra_GX430t".into(),
            is_default: true,
            dpi: Some(203),
            port: Some("USB001".into()),
            is_local: Some(true),
            is_network: Some(false),
            is_virtual: Some(false),
            availability: Default::default(),
        };
        let trays = [PrinterTrayInfo {
            id: "Tray1".into(),
            name: "Tray 1".into(),
        }];

        let lines = visible_text(&build_test_page(
            &config,
            "Zebra_GX430t",
            &paper,
            &sample_device(),
            Some(&printer),
            &trays,
        ));

        assert!(lines.iter().all(|line| !line.contains("YinShu")));
        assert!(lines.iter().any(|line| line.contains("Yinshu")));
        assert!(lines.iter().any(|line| line.contains("Test page")));
        assert!(lines.iter().any(|line| line.contains("Zebra_GX430t")));
        assert!(lines.iter().any(|line| line.contains("Via")));
        assert!(lines.iter().any(|line| line.contains("Not reported")));
        assert!(lines.iter().any(|line| line.contains("Tray 1")));
        assert!(lines.iter().any(|line| line.contains("203 dpi")));
        assert!(lines.iter().any(|line| line.contains("USB001")));
        assert!(lines.iter().any(|line| line.contains("192.168.1.23:17890")));
        assert!(lines.iter().any(|line| line.contains("https://erp.example.com")));
        assert!(lines.iter().any(|line| line.contains("60 x 40 mm")));
        assert!(lines.iter().all(|line| line.chars().all(|c| c.is_ascii())));
    }

    #[tokio::test]
    async fn test_print_writes_submitted_history() {
        let config = test_print_config();
        let state =
            AgentState::with_printing(config.clone(), Box::new(TestPrintBackend::Submitted))
                .with_task_history_store(TaskHistoryStore::open_in_memory().unwrap());

        print_test_page_with_config(&state, config).await.unwrap();

        let jobs = state
            .task_history
            .as_ref()
            .unwrap()
            .recent_jobs(10)
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].source, TaskHistorySource::Test);
        assert_eq!(jobs[0].current_status, TaskHistoryStatus::Submitted);
        assert_eq!(jobs[0].printer_name.as_deref(), Some("CITIZEN_CL_S700"));
        assert_eq!(jobs[0].paper_name.as_deref(), Some("83 x 152 mm"));
        assert_eq!(jobs[0].copies, Some(1));
    }

    #[tokio::test]
    async fn test_print_writes_failed_history() {
        let config = test_print_config();
        let state = AgentState::with_printing(config.clone(), Box::new(TestPrintBackend::Failed))
            .with_task_history_store(TaskHistoryStore::open_in_memory().unwrap());

        let error = print_test_page_with_config(&state, config)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("I/O error"));

        let jobs = state
            .task_history
            .as_ref()
            .unwrap()
            .recent_jobs(10)
            .unwrap();
        assert_eq!(jobs[0].source, TaskHistorySource::Test);
        assert_eq!(jobs[0].current_status, TaskHistoryStatus::Failed);
    }

    #[tokio::test]
    async fn test_print_skips_history_when_printer_offline() {
        let config = test_print_config();
        let state = AgentState::with_printing(config.clone(), Box::new(TestPrintBackend::Offline))
            .with_task_history_store(TaskHistoryStore::open_in_memory().unwrap());

        let error = print_test_page_with_config(&state, config)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("printer offline"));

        let jobs = state
            .task_history
            .as_ref()
            .unwrap()
            .recent_jobs(10)
            .unwrap();
        assert!(jobs.is_empty());
    }

    fn test_print_config() -> AgentConfig {
        let mut config = AgentConfig::default();
        config.printing.default_printer = Some("CITIZEN_CL_S700".into());
        config.printing.default_paper = Some(EffectivePaper {
            width_mm: 83.0,
            height_mm: 152.0,
        });
        config
    }

    enum TestPrintBackend {
        Submitted,
        Failed,
        Offline,
    }

    impl PrintBackend for TestPrintBackend {
        fn list_printers(&self) -> PrintResult<Vec<PrinterInfo>> {
            Ok(vec![])
        }

        fn list_papers(&self, _printer_name: &str) -> PrintResult<Vec<PaperInfo>> {
            Ok(vec![])
        }

        fn print_pdf(&self, _path: &Path, _options: &PrintOptions) -> PrintResult<PrintSubmission> {
            match self {
                Self::Submitted => Ok(PrintSubmission {
                    submitted_at: "2026-09-15T02:00:00Z".into(),
                    backend: "test".into(),
                    system_job_id: None,
                    tracking_supported: false,
                }),
                Self::Failed => Err(PrintError::CommandFailed {
                    command: "lp".into(),
                    message: "I/O error".into(),
                }),
                Self::Offline => Err(PrintError::PrinterOffline),
            }
        }

        fn print_raw(
            &self,
            _data: &[u8],
            _options: &RawPrintOptions,
        ) -> PrintResult<PrintSubmission> {
            Err(PrintError::UnsupportedPlatform)
        }
    }
}
