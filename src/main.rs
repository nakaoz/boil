mod boil;
mod bot;
mod cli;
mod config;
mod core;
mod service;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "redial", about = "Boil.network 换 IP 工具", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 查看当前 IP 和今日剩余额度
    Status,
    /// 换 IP（重拨）
    Change,
    /// 启动 Telegram 机器人（需配置 TG）
    Bot,
    /// 重新运行配置向导
    Setup,
    /// 系统服务管理
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// 安装并启用 systemd 服务
    Install,
    /// 停止并卸载 systemd 服务
    Uninstall,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let cli = Cli::parse();

    match cli.command {
        None => {
            let config = config::load_or_setup().await?;
            if config.has_tg() {
                println!("启动 Telegram 机器人...");
                bot::run(config).await?;
            } else {
                println!("提示: 未配置 Telegram，可直接使用以下命令：");
                println!("  redial status   查看当前 IP");
                println!("  redial change   换 IP");
                println!("  redial setup    重新配置（含 TG）");
            }
        }
        Some(Commands::Status) => {
            let config = config::load_or_setup().await?;
            cli::cmd_status(&config).await?;
        }
        Some(Commands::Change) => {
            let config = config::load_or_setup().await?;
            cli::cmd_change(&config).await?;
        }
        Some(Commands::Bot) => {
            let config = config::load_or_setup().await?;
            bot::run(config).await?;
        }
        Some(Commands::Setup) => {
            config::run_setup_wizard().await?;
        }
        Some(Commands::Service { action }) => match action {
            ServiceAction::Install => service::install()?,
            ServiceAction::Uninstall => service::uninstall()?,
        },
    }

    Ok(())
}
