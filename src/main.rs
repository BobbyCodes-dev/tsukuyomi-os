mod app;
mod launch_external;
mod ollama_setup;
mod rustdesk;
mod scan;
mod screens;
mod store;
mod ui;
mod uninstall;
mod vm;

use std::io::stdout;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

#[derive(Parser)]
#[command(name = "tsukuyomi", about = "Tsukuyomi OS - terminal-based personal OS shell")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Uninstall {
        #[arg(long)]
        keep_vms: bool,
        #[arg(long)]
        yes: bool,
    },
    AiAgent,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Uninstall { keep_vms, yes }) => {
            uninstall::nuke(uninstall::UninstallArgs { keep_vms, yes })
        }
        Some(Commands::AiAgent) => run_tui(true),
        None => run_tui(false),
    }
}

fn run_tui(start_ai_agent: bool) -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        default_panic(info);
    }));

    let mut app = app::App::new(start_ai_agent)?;
    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
