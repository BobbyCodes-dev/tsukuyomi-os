mod app;
mod launch_external;
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
    /// Permanently delete all local Tsukuyomi OS data.
    Uninstall {
        #[arg(long)]
        keep_vms: bool,
        #[arg(long)]
        yes: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Uninstall { keep_vms, yes }) => {
            uninstall::nuke(uninstall::UninstallArgs { keep_vms, yes })
        }
        None => run_tui(),
    }
}

fn run_tui() -> Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    // Restore the terminal even if the app panics mid-render, so a crash
    // doesn't leave the user's shell stuck in raw/alternate-screen mode.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        default_panic(info);
    }));

    let mut app = app::App::new()?;
    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
