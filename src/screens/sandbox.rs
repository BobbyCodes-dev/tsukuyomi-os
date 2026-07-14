use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::{self, settings};
use crate::ui::{theme, widgets};
use crate::vm;
use crate::vm::builder::BuildEvent;
use crate::vm::network::NetworkMode;

const VM_NAME: &str = "TsukuyomiOS";

pub struct SandboxState {
    pub backends: Vec<vm::VMBackend>,
    pub selected: usize,
    pub log: widgets::LogPanel,
    pub building: bool,
    build_rx: Option<Receiver<BuildEvent>>,
}

impl SandboxState {
    pub fn new() -> Self {
        let backends = vm::detect_backends();
        let selected = match vm::choose_backend(&backends, None) {
            Some(best) => backends.iter().position(|b| b.id == best.id).unwrap_or(0),
            None => 0,
        };
        let mut log = widgets::LogPanel::new(200);
        log.push("Detecting VM backends...");
        log.push(vm::suggest_action(&backends));
        SandboxState { backends, selected, log, building: false, build_rx: None }
    }

    pub fn poll_build(&mut self) {
        let Some(rx) = &self.build_rx else { return };
        loop {
            match rx.try_recv() {
                Ok(BuildEvent::Status(s)) => self.log.push(s),
                Ok(BuildEvent::Done(vdi)) => {
                    self.log.push(format!("VM disk ready at {}.", vdi.display()));
                    self.building = false;
                    self.build_rx = None;
                    finalize_and_launch(self, &vdi);
                    break;
                }
                Ok(BuildEvent::Error(e)) => {
                    self.log.push(format!("VM build failed: {e}"));
                    self.building = false;
                    self.build_rx = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.building = false;
                    self.build_rx = None;
                    break;
                }
            }
        }
    }
}

fn finalize_and_launch(state: &mut SandboxState, vdi: &std::path::Path) {
    if let Err(e) = vm::launch::create_virtualbox_vm(VM_NAME, vdi) {
        state.log.push(format!("VM setup: {e}"));
        return;
    }
    match vm::launch::launch_vm("virtualbox", Some(VM_NAME), None) {
        Ok(()) => state.log.push("Launched VirtualBox sandbox.".to_string()),
        Err(e) => state.log.push(format!("Failed to launch VirtualBox: {e}")),
    }
}

pub fn draw(frame: &mut Frame, area: Rect, state: &SandboxState) {
    let rect = widgets::centered_fixed(90, area.height.min(28), area);
    let block = widgets::form_block("Tsukuyomi Sandbox");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::styled(
            "Select a VM backend to launch an isolated Windows environment.",
            theme::subtitle_style(),
        )),
        chunks[0],
    );

    let rows: Vec<Row> = state
        .backends
        .iter()
        .map(|b| {
            Row::new(vec![
                b.name.clone(),
                if b.available { "Yes".to_string() } else { "No".to_string() },
                b.reason.clone(),
            ])
        })
        .collect();
    let table = Table::new(rows, [Constraint::Length(18), Constraint::Length(6), Constraint::Min(20)])
        .header(Row::new(vec!["Backend", "Available", "Notes"]).style(theme::title_style()))
        .row_highlight_style(theme::focused_field_style())
        .highlight_symbol("> ");
    let mut table_state = TableState::default().with_selected(Some(state.selected));
    frame.render_stateful_widget(table, chunks[1], &mut table_state);

    let visible = chunks[2].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget = Paragraph::new(all[start..].to_vec())
        .block(widgets::log_block("Log"))
        .wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[2]);

    let hint = if state.building {
        "Building VM (download + unattended install)... please wait.  Esc: Back"
    } else {
        "Enter: Launch  Esc: Back"
    };
    frame.render_widget(Paragraph::new(Line::styled(hint, theme::hint_style())), chunks[3]);
}

fn launch(state: &mut SandboxState) {
    if state.building {
        state.log.push("A VM build is already in progress.".to_string());
        return;
    }

    let backend = state.backends[state.selected].clone();
    if !backend.available {
        state.log.push(format!("{} is not available on this machine.", backend.name));
        return;
    }

    if backend.id == "virtualbox" {
        let vm_dir = match store::ensure_data_dir() {
            Ok(d) => d.join("vm"),
            Err(e) => {
                state.log.push(format!("{e}"));
                return;
            }
        };
        let vdi_path = vm_dir.join(format!("{VM_NAME}.vdi"));
        if vdi_path.exists() {
            finalize_and_launch(state, &vdi_path);
            return;
        }

        let network = NetworkMode::from_id(&settings::load_settings().vm_network_mode);
        state.log.push(format!(
            "No sandbox disk found. Starting build: download Alpine 'virt' ISO, verify checksum, \
             create VM, and run unattended install (network mode: {}).",
            network.label()
        ));
        let (tx, rx) = mpsc::channel();
        state.build_rx = Some(rx);
        state.building = true;
        let vm_name = VM_NAME.to_string();
        std::thread::spawn(move || {
            let done_tx = tx.clone();
            match vm::builder::build_or_download_vm(&vm_dir, &vm_name, network, tx) {
                Ok(vdi) => {
                    let _ = done_tx.send(BuildEvent::Done(vdi));
                }
                Err(e) => {
                    let _ = done_tx.send(BuildEvent::Error(e.to_string()));
                }
            }
        });
        return;
    }

    match vm::launch::launch_vm(&backend.id, None, None) {
        Ok(()) => state.log.push(format!("Launched {}.", backend.name)),
        Err(e) => state.log.push(format!("Failed to launch {}: {e}", backend.name)),
    }
}

pub fn handle_key(state: &mut SandboxState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            state.selected = if state.selected == 0 { state.backends.len() - 1 } else { state.selected - 1 };
            Action::None
        }
        KeyCode::Down => {
            state.selected = (state.selected + 1) % state.backends.len();
            Action::None
        }
        KeyCode::Enter => {
            launch(state);
            Action::None
        }
        _ => Action::None,
    }
}
