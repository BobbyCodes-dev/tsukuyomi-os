use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::netryx::{self, NetryxResult};
use crate::ui::{theme, widgets};

enum Mode {
    List,
    New,
    Detail,
    Running,
}

pub struct NetryxState {
    user_id: i64,
    results: Vec<NetryxResult>,
    selected: usize,
    mode: Mode,
    image_path: widgets::TextField,
    status: String,
    running: bool,
    run_rx: Option<Receiver<(String, String, String, String, String)>>,
    log: widgets::LogPanel,
}

impl NetryxState {
    pub fn new(user_id: i64) -> Self {
        let mut log = widgets::LogPanel::new(200);
        log.push("Netryx Astra V2: AI-powered image geolocation.".to_string());
        log.push("Uses MegaLoc + MASt3R models. Requires Python 3.10+.".to_string());
        log.push("Press 'n' to analyze a new image.".to_string());
        let mut state = NetryxState {
            user_id,
            results: Vec::new(),
            selected: 0,
            mode: Mode::List,
            image_path: widgets::TextField::new(),
            status: String::new(),
            running: false,
            run_rx: None,
            log,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match netryx::list_results(self.user_id) {
            Ok(results) => {
                self.results = results;
                if self.selected >= self.results.len() {
                    self.selected = self.results.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading results: {e}"),
        }
    }

    pub fn poll_run(&mut self) {
        if !self.running {
            return;
        }
        let Some(rx) = &self.run_rx else { return };
        match rx.try_recv() {
            Ok((image_path, location, confidence, lat, lon)) => {
                match netryx::add_result(
                    self.user_id,
                    &image_path,
                    &location,
                    &confidence,
                    &lat,
                    &lon,
                    "{}",
                ) {
                    Ok(_) => {
                        self.log.push(format!("Geolocation result: {location} (confidence: {confidence})"));
                        self.status = "Analysis complete.".to_string();
                    }
                    Err(e) => self.status = format!("Error saving result: {e}"),
                }
                self.running = false;
                self.run_rx = None;
                self.mode = Mode::List;
                self.refresh();
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.log.push("Netryx analysis thread disconnected.".to_string());
                self.running = false;
                self.run_rx = None;
                self.mode = Mode::List;
            }
        }
    }
}

fn start_analysis(state: &mut NetryxState) {
    let path = state.image_path.value.trim().to_string();
    if path.is_empty() {
        state.status = "Image path is required.".to_string();
        return;
    }
    state.status = "Running image geolocation...".to_string();
    state.log.push(format!("Analyzing: {path}"));
    state.running = true;
    state.mode = Mode::Running;

    let (tx, rx) = mpsc::channel();
    state.run_rx = Some(rx);

    std::thread::spawn(move || {
        let result = run_netryx_analysis(&path);
        let _ = tx.send(result);
    });
}

/// Runs the Netryx Astra V2 geolocation pipeline.
/// Invokes the Python script to analyze the image and predict location.
/// Returns (image_path, predicted_location, confidence, latitude, longitude).
fn run_netryx_analysis(image_path: &str) -> (String, String, String, String, String) {
    use std::process::Command;

    // Try the netryx CLI / Python module
    let py_script = format!(
        r#"
import sys
try:
    from netryx_astra import geolocate
    result = geolocate(r"{}")
    if result:
        print(result.get("location", "Unknown"))
        print(result.get("confidence", "N/A"))
        print(result.get("latitude", ""))
        print(result.get("longitude", ""))
    else:
        print("No result")
        print("0.0")
        print("")
        print("")
except ImportError:
    print("MODULE_NOT_FOUND")
    print("0.0")
    print("")
    print("")
except Exception as e:
    print(f"ERROR: {{e}}")
    print("0.0")
    print("")
    print("")
"#,
        image_path.replace('"', "")
    );

    let output = Command::new("python")
        .args(["-c", &py_script])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let lines: Vec<&str> = stdout.lines().collect();
            if lines.len() >= 4 {
                let location = lines[0].to_string();
                let confidence = lines[1].to_string();
                let lat = lines[2].to_string();
                let lon = lines[3].to_string();

                if location == "MODULE_NOT_FOUND" {
                    return (
                        image_path.to_string(),
                        "Module not installed".to_string(),
                        "0.0".to_string(),
                        "".to_string(),
                        "".to_string(),
                    );
                }

                return (image_path.to_string(), location, confidence, lat, lon);
            }
            (
                image_path.to_string(),
                "Parse error".to_string(),
                "0.0".to_string(),
                "".to_string(),
                "".to_string(),
            )
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            (
                image_path.to_string(),
                format!("Error: {stderr}"),
                "0.0".to_string(),
                "".to_string(),
                "".to_string(),
            )
        }
        Err(e) => (
            image_path.to_string(),
            format!("Failed to execute: {e}"),
            "0.0".to_string(),
            "".to_string(),
            "".to_string(),
        ),
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &NetryxState) {
    let rect = widgets::centered_fixed(100, area.height.min(30), area);
    let block = widgets::form_block("Netryx Astra V2 - Image Geolocation");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::New => draw_new(frame, inner, state),
        Mode::Detail => draw_detail(frame, inner, state),
        Mode::Running => draw_running(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &NetryxState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(8), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .results
        .iter()
        .map(|r| {
            Row::new(vec![
                r.image_path.clone(),
                r.predicted_location.clone(),
                r.confidence.clone(),
                r.created_at.clone(),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [Constraint::Min(30), Constraint::Min(20), Constraint::Length(12), Constraint::Length(20)],
    )
    .header(Row::new(vec!["Image", "Predicted Location", "Confidence", "Analyzed"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.results.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget =
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Status")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let mut lines = vec![Line::styled(
        "n: new analysis  Enter: detail  d: delete  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[2]);
}

fn draw_new(frame: &mut Frame, area: Rect, state: &NetryxState) {
    let mut lines = vec![
        Line::styled("New Image Geolocation", theme::title_style()),
        Line::raw(""),
        field_line("Image path (local file)", state.image_path.display(), true),
        Line::raw(""),
        Line::styled("Enter: analyze  Esc: cancel", theme::hint_style()),
        Line::styled(
            "Predicts geographic location from image using AI models (MegaLoc + MASt3R).",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_detail(frame: &mut Frame, area: Rect, state: &NetryxState) {
    let item = state.results.get(state.selected);
    let mut lines = vec![Line::styled("Geolocation Result", theme::title_style()), Line::raw("")];
    if let Some(r) = item {
        lines.push(Line::from(vec![Span::styled("Image: ", theme::title_style()), Span::raw(r.image_path.clone())]));
        lines.push(Line::from(vec![Span::styled("Location: ", theme::title_style()), Span::raw(r.predicted_location.clone())]));
        lines.push(Line::from(vec![Span::styled("Confidence: ", theme::title_style()), Span::raw(r.confidence.clone())]));
        if !r.latitude.is_empty() || !r.longitude.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Coordinates: ", theme::title_style()),
                Span::raw(format!("{}, {}", r.latitude, r.longitude)),
            ]));
        }
        lines.push(Line::from(vec![Span::styled("Analyzed: ", theme::title_style()), Span::raw(r.created_at.clone())]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("Esc: back", theme::hint_style()));
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_running(frame: &mut Frame, area: Rect, state: &NetryxState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let header = vec![
        Line::styled("Analyzing Image...", theme::title_style()),
        Line::raw(""),
        Line::from(vec![
            Span::styled("Image: ", theme::title_style()),
            Span::raw(state.image_path.value.clone()),
        ]),
    ];
    frame.render_widget(Paragraph::new(header), chunks[0]);

    let visible = chunks[1].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.log.lines().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let log_widget =
        Paragraph::new(all[start..].to_vec()).block(widgets::log_block("Analysis Log")).wrap(Wrap { trim: false });
    frame.render_widget(log_widget, chunks[1]);

    let lines = vec![Line::styled("Press Esc to return (analysis continues in background)", theme::hint_style())];
    frame.render_widget(Paragraph::new(lines), chunks[2]);
}

fn handle_list_key(state: &mut NetryxState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.results.is_empty() {
                state.selected = if state.selected == 0 { state.results.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.results.is_empty() {
                state.selected = (state.selected + 1) % state.results.len();
            }
            Action::None
        }
        KeyCode::Char('n') => {
            state.image_path = widgets::TextField::new();
            state.mode = Mode::New;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            if !state.results.is_empty() {
                state.mode = Mode::Detail;
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(r) = state.results.get(state.selected) {
                let id = r.id;
                match netryx::delete_result(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Result deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting result: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_new_key(state: &mut NetryxState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            start_analysis(state);
            Action::None
        }
        KeyCode::Backspace => {
            state.image_path.backspace();
            Action::None
        }
        KeyCode::Char(c) => {
            state.image_path.push_char(c);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_running_key(state: &mut NetryxState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status = "Analysis running in background...".to_string();
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut NetryxState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::New => handle_new_key(state, key),
        Mode::Detail => {
            if key.code == KeyCode::Esc {
                state.mode = Mode::List;
            }
            Action::None
        }
        Mode::Running => handle_running_key(state, key),
    }
}