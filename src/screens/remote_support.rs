use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::rustdesk::{self, DownloadEvent};
use crate::store::remote_support::{self, Bookmark};
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 2;

enum Mode {
    List,
    Edit(Option<i64>),
    QuickConnect,
}

enum PendingAction {
    Host,
    Connect(String),
}

pub struct RemoteSupportState {
    user_id: i64,
    entries: Vec<Bookmark>,
    selected: usize,
    mode: Mode,
    label: widgets::TextField,
    remote_id: widgets::TextField,
    focus: usize,
    quick_id: widgets::TextField,
    log: widgets::LogPanel,
    downloading: bool,
    download_rx: Option<Receiver<DownloadEvent>>,
    pending_action: Option<PendingAction>,
    status: String,
}

impl RemoteSupportState {
    pub fn new(user_id: i64) -> Self {
        let mut log = widgets::LogPanel::new(200);
        if rustdesk::is_installed() {
            log.push(format!("RustDesk client found at {}", rustdesk::exe_path().display()));
        } else {
            log.push("RustDesk client not installed yet. It will be downloaded on first use.".to_string());
        }
        log.push(
            "Using RustDesk's public rendezvous server by default. Configure a self-hosted hbbs/hbbr \
             address from RustDesk's own settings UI if you need full control."
                .to_string(),
        );
        let mut state = RemoteSupportState {
            user_id,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::List,
            label: widgets::TextField::new(),
            remote_id: widgets::TextField::new(),
            focus: 0,
            quick_id: widgets::TextField::new(),
            log,
            downloading: false,
            download_rx: None,
            pending_action: None,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match remote_support::list_bookmarks(self.user_id) {
            Ok(entries) => {
                self.entries = entries;
                if self.selected >= self.entries.len() {
                    self.selected = self.entries.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading bookmarks: {e}"),
        }
    }

    fn clear_form(&mut self) {
        self.label = widgets::TextField::new();
        self.remote_id = widgets::TextField::new();
        self.focus = 0;
    }

    pub fn poll_download(&mut self) {
        let Some(rx) = &self.download_rx else { return };
        loop {
            match rx.try_recv() {
                Ok(DownloadEvent::Status(s)) => self.log.push(s),
                Ok(DownloadEvent::Done(path)) => {
                    self.log.push(format!("RustDesk ready at {}", path.display()));
                    self.downloading = false;
                    self.download_rx = None;
                    if let Some(action) = self.pending_action.take() {
                        perform_launch(self, action);
                    }
                    break;
                }
                Ok(DownloadEvent::Error(e)) => {
                    self.log.push(format!("RustDesk setup failed: {e}"));
                    self.downloading = false;
                    self.download_rx = None;
                    self.pending_action = None;
                    break;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.downloading = false;
                    self.download_rx = None;
                    break;
                }
            }
        }
    }
}

fn perform_launch(state: &mut RemoteSupportState, action: PendingAction) {
    match action {
        PendingAction::Host => match rustdesk::launch_host() {
            Ok(()) => state.log.push(
                "Launched RustDesk in host mode. Share the ID and password it shows with whoever is \
                 connecting in to help."
                    .to_string(),
            ),
            Err(e) => state.log.push(format!("Failed to launch RustDesk: {e}")),
        },
        PendingAction::Connect(id) => match rustdesk::launch_connect(&id) {
            Ok(()) => state.log.push(format!("Launching RustDesk, connecting to {id}...")),
            Err(e) => state.log.push(format!("Failed to launch RustDesk: {e}")),
        },
    }
}

fn ensure_and_run(state: &mut RemoteSupportState, action: PendingAction) {
    if rustdesk::is_installed() {
        perform_launch(state, action);
        return;
    }
    if state.downloading {
        state.pending_action = Some(action);
        state.log.push("RustDesk download already in progress...".to_string());
        return;
    }
    state.pending_action = Some(action);
    state.downloading = true;
    state.log.push("RustDesk client not found. Downloading the latest portable build from GitHub...".to_string());
    let (tx, rx) = mpsc::channel();
    state.download_rx = Some(rx);
    std::thread::spawn(move || {
        let done_tx = tx.clone();
        match rustdesk::ensure_rustdesk(tx) {
            Ok(path) => {
                let _ = done_tx.send(DownloadEvent::Done(path));
            }
            Err(e) => {
                let _ = done_tx.send(DownloadEvent::Error(e.to_string()));
            }
        }
    });
}

fn connect_selected(state: &mut RemoteSupportState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    ensure_and_run(state, PendingAction::Connect(entry.remote_id));
}

fn save_entry(state: &mut RemoteSupportState) {
    if state.label.value.trim().is_empty() || state.remote_id.value.trim().is_empty() {
        state.status = "Label and Remote ID are required.".to_string();
        return;
    }
    let result = match state.mode {
        Mode::Edit(Some(id)) => remote_support::update_bookmark(
            state.user_id,
            id,
            state.label.value.trim(),
            state.remote_id.value.trim(),
        ),
        _ => remote_support::add_bookmark(state.user_id, state.label.value.trim(), state.remote_id.value.trim()),
    };
    match result {
        Ok(()) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving bookmark: {e}"),
    }
}

fn load_selected_into_form(state: &mut RemoteSupportState) {
    let Some(entry) = state.entries.get(state.selected).cloned() else { return };
    state.label = widgets::TextField::with_value(entry.label.clone());
    state.remote_id = widgets::TextField::with_value(entry.remote_id.clone());
    state.focus = 0;
    state.mode = Mode::Edit(Some(entry.id));
    state.status.clear();
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &RemoteSupportState) {
    let rect = widgets::centered_fixed(96, area.height.min(28), area);
    let block = widgets::form_block("Remote Support");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
        Mode::QuickConnect => draw_quick_connect(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &RemoteSupportState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(8), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state.entries.iter().map(|b| Row::new(vec![b.label.clone(), b.remote_id.clone()])).collect();
    let table = Table::new(rows, [Constraint::Length(24), Constraint::Min(20)])
        .header(Row::new(vec!["Label", "Remote ID"]).style(theme::title_style()))
        .row_highlight_style(theme::focused_field_style())
        .highlight_symbol("> ")
        .block(widgets::form_block("Saved Bookmarks"));
    let mut table_state =
        TableState::default().with_selected(if state.entries.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget =
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Status")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let mut lines = vec![Line::styled(
        "h: host mode  Enter: connect  c: quick connect  a: add  e: edit  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[2]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &RemoteSupportState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Bookmark" } else { "New Bookmark" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Label", state.label.display(), state.focus == 0),
        field_line("Remote ID", state.remote_id.display(), state.focus == 1),
        Line::raw(""),
        Line::styled("Tab: move  Enter: save  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_quick_connect(frame: &mut Frame, area: Rect, state: &RemoteSupportState) {
    let mut lines = vec![
        Line::styled("Quick Connect", theme::title_style()),
        Line::raw(""),
        field_line("Remote ID", state.quick_id.display(), true),
        Line::raw(""),
        Line::styled("Enter: connect  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn handle_list_key(state: &mut RemoteSupportState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.entries.is_empty() {
                state.selected = if state.selected == 0 { state.entries.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.entries.is_empty() {
                state.selected = (state.selected + 1) % state.entries.len();
            }
            Action::None
        }
        KeyCode::Char('a') => {
            state.clear_form();
            state.mode = Mode::Edit(None);
            state.status.clear();
            Action::None
        }
        KeyCode::Char('e') => {
            load_selected_into_form(state);
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(entry) = state.entries.get(state.selected) {
                let id = entry.id;
                match remote_support::delete_bookmark(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Bookmark deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting bookmark: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Char('h') => {
            ensure_and_run(state, PendingAction::Host);
            Action::None
        }
        KeyCode::Char('c') => {
            state.quick_id = widgets::TextField::new();
            state.mode = Mode::QuickConnect;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            connect_selected(state);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut RemoteSupportState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Tab | KeyCode::Down => {
            state.focus = (state.focus + 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = (state.focus + FIELD_COUNT - 1) % FIELD_COUNT;
            Action::None
        }
        KeyCode::Enter => {
            save_entry(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.label.backspace(),
                1 => state.remote_id.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.label.push_char(c),
                1 => state.remote_id.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_quick_connect_key(state: &mut RemoteSupportState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            let id = state.quick_id.value.trim().to_string();
            if id.is_empty() {
                state.status = "Enter a Remote ID first.".to_string();
                return Action::None;
            }
            state.mode = Mode::List;
            ensure_and_run(state, PendingAction::Connect(id));
            Action::None
        }
        KeyCode::Backspace => {
            state.quick_id.backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.quick_id.push_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut RemoteSupportState, key: KeyEvent) -> Action {
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::Edit(_) => handle_edit_key(state, key),
        Mode::QuickConnect => handle_quick_connect_key(state, key),
    }
}
