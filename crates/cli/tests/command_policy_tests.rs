use yinshu_cli::{Command, CommandPolicy};

#[test]
fn mutating_commands_prefer_the_running_agent() {
    assert_eq!(
        Command::ClearTaskHistory.policy(),
        CommandPolicy::OnlinePreferred
    );
}

#[test]
fn status_requires_the_running_agent() {
    assert_eq!(Command::Status.policy(), CommandPolicy::OnlineOnly);
}

#[test]
fn config_validation_can_run_offline() {
    assert_eq!(
        Command::ValidateConfig { path: None }.policy(),
        CommandPolicy::OfflineAllowed
    );
}

#[test]
fn diagnostics_export_can_run_offline() {
    assert_eq!(
        Command::ExportDiagnostics {
            path: None,
            doctor_json: None,
        }
        .policy(),
        CommandPolicy::OfflineAllowed
    );
}
