use std::{net::SocketAddr, sync::Arc};

use yinshu_cli::{
    client::LocalClientExecutor,
    parser::{cli_display_output_from, run_cli_from},
    CommandExecutor, CommandService, ProductCommandAdapter, TerminalInteraction,
    UnsupportedProductCommandAdapter,
};
use yinshu_runtime::{ipc, RuntimeBuilder, RuntimeCommandExecutor, RuntimePaths};

use crate::product_cli::DesktopProductCommandAdapter;

/// 从当前进程参数运行两个产品共享的功能 CLI。
pub fn run_cli_from_env() -> i32 {
    let argv = std::env::args_os().collect::<Vec<_>>();
    if let Some(output) = cli_display_output_from(argv.clone()) {
        print!("{}", output.stdout);
        return output.exit_code;
    }
    let diagnose = argv.get(1).and_then(|arg| arg.to_str()) == Some("diagnose");
    let service = if diagnose {
        yinshu_cli::diagnose_command_service()
    } else {
        let read_only = argv.get(1).and_then(|arg| arg.to_str()) == Some("doctor");
        match build_service(read_only) {
            Ok(service) => service,
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    };
    let product: Arc<dyn ProductCommandAdapter> = if diagnose {
        Arc::new(UnsupportedProductCommandAdapter::new(
            "diagnose does not use desktop autostart or language commands",
        ))
    } else {
        match DesktopProductCommandAdapter::new(service.clone()) {
            Ok(product) => Arc::new(product),
            Err(error) => {
                eprintln!("{error}");
                return 1;
            }
        }
    };
    let runtime = tokio::runtime::Runtime::new().expect("create CLI runtime");
    match runtime.block_on(run_cli_from(
        argv,
        service,
        product,
        Arc::new(TerminalInteraction),
    )) {
        Ok(output) => {
            if !output.stdout.is_empty() {
                print!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }
            output.exit_code
        }
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn build_service(read_only: bool) -> Result<Arc<CommandService>, Box<dyn std::error::Error>> {
    let config_path = yinshu_core::config::cli_config_path()?;
    let data_dir = yinshu_core::config::cli_data_dir()?;
    let runtime_dir = data_dir.join("run");
    let state = if read_only {
        let config = yinshu_core::config::AgentConfig::load(&config_path)?;
        yinshu_runtime::state::AgentState::with_config_path_and_printing(
            config,
            config_path,
            yinshu_runtime::printing::default_backend(),
        )
    } else {
        RuntimeBuilder::new(RuntimePaths::new(
            config_path,
            data_dir,
            runtime_dir.clone(),
        ))
        .build()?
        .state()
    };
    let offline: Arc<dyn CommandExecutor> = Arc::new(RuntimeCommandExecutor::new(
        state,
        SocketAddr::from(([127, 0, 0, 1], 0)),
    ));
    let online: Arc<dyn CommandExecutor> =
        Arc::new(LocalClientExecutor::new(ipc::socket_path(&runtime_dir)));
    Ok(Arc::new(CommandService::new(Some(online), offline)))
}
