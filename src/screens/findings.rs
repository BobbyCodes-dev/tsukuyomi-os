use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::engagements;
use crate::store::findings::{self, Finding};
use crate::store::reports;
use crate::ui::{theme, widgets};

const FIELD_COUNT: usize = 8;
const SEVERITIES: [&str; 5] = ["Critical", "High", "Medium", "Low", "Info"];
const STATUSES: [&str; 4] = ["Open", "In Progress", "Resolved", "Risk Accepted"];

enum ExportMode {
    None,
    FindingsOnly,
    ClientReport,
}

enum Mode {
    List,
    Edit(Option<i64>),
    View,
    Report,
    Export,
}

pub struct FindingsState {
    user_id: i64,
    findings: Vec<Finding>,
    engagements: Vec<(i64, String)>,
    selected: usize,
    mode: Mode,
    title: widgets::TextField,
    engagement_idx: usize,
    severity_idx: usize,
    status_idx: usize,
    cvss: widgets::TextField,
    description: widgets::TextField,
    remediation: widgets::TextField,
    affected_assets: widgets::TextField,
    evidence_ids: widgets::TextField,
    cve_ids: widgets::TextField,
    reported_at: widgets::TextField,
    focus: usize,
    report_text: String,
    export_path: widgets::TextField,
    export_mode: ExportMode,
    status: String,
}

impl FindingsState {
    pub fn new(user_id: i64) -> Self {
        let mut state = FindingsState {
            user_id,
            findings: Vec::new(),
            engagements: Vec::new(),
            selected: 0,
            mode: Mode::List,
            title: widgets::TextField::new(),
            engagement_idx: 0,
            severity_idx: 0,
            status_idx: 0,
            cvss: widgets::TextField::new(),
            description: widgets::TextField::new(),
            remediation: widgets::TextField::new(),
            affected_assets: widgets::TextField::new(),
            evidence_ids: widgets::TextField::new(),
            cve_ids: widgets::TextField::new(),
            reported_at: widgets::TextField::new(),
            focus: 0,
            report_text: String::new(),
            export_path: widgets::TextField::new(),
            export_mode: ExportMode::None,
            status: String::new(),
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        match findings::list_findings(self.user_id) {
            Ok(items) => {
                self.findings = items;
                if self.selected >= self.findings.len() {
                    self.selected = self.findings.len().saturating_sub(1);
                }
            }
            Err(e) => self.status = format!("Error loading findings: {e}"),
        }
        self.engagements = engagements::list_engagement_labels(self.user_id).unwrap_or_default();
    }

    fn clear_form(&mut self) {
        self.title = widgets::TextField::new();
        self.engagement_idx = 0;
        self.severity_idx = 0;
        self.status_idx = 0;
        self.cvss = widgets::TextField::new();
        self.description = widgets::TextField::new();
        self.remediation = widgets::TextField::new();
        self.affected_assets = widgets::TextField::new();
        self.evidence_ids = widgets::TextField::new();
        self.cve_ids = widgets::TextField::new();
        self.reported_at = widgets::TextField::with_value(today());
        self.focus = 0;
    }

    fn load_selected_into_form(&mut self) {
        if let Some(f) = self.findings.get(self.selected) {
            self.title = widgets::TextField::with_value(f.title.clone());
            self.engagement_idx = self.engagements.iter().position(|(id, _)| *id == f.engagement_id).unwrap_or(0);
            self.severity_idx = SEVERITIES.iter().position(|s| *s == f.severity).unwrap_or(4);
            self.status_idx = STATUSES.iter().position(|s| *s == f.status).unwrap_or(0);
            self.cvss = widgets::TextField::with_value(f.cvss.clone());
            self.description = widgets::TextField::with_value(f.description.clone());
            self.remediation = widgets::TextField::with_value(f.remediation.clone());
            self.affected_assets = widgets::TextField::with_value(f.affected_assets.clone());
            self.evidence_ids = widgets::TextField::with_value(f.evidence_ids.clone());
            self.cve_ids = widgets::TextField::with_value(f.cve_ids.clone());
            self.reported_at = widgets::TextField::with_value(f.reported_at.clone());
            self.focus = 0;
        }
    }

    fn build_report(&mut self) {
        let mut md = String::from("# Findings Report

");
        md.push_str(&format!("Generated: {}

", today()));
        for f in &self.findings {
            md.push_str(&format!("## {}
", f.title));
            md.push_str(&format!("- **Severity:** {}  
", f.severity));
            md.push_str(&format!("- **Status:** {}  
", f.status));
            md.push_str(&format!("- **CVSS:** {}  
", f.cvss));
            md.push_str(&format!("- **Reported:** {}  

", f.reported_at));
            if !f.description.is_empty() {
                md.push_str("### Description

");
                md.push_str(&f.description);
                md.push_str("

");
            }
            if !f.remediation.is_empty() {
                md.push_str("### Remediation

");
                md.push_str(&f.remediation);
                md.push_str("

");
            }
            if !f.affected_assets.is_empty() {
                md.push_str(&format!("### Affected Assets

{}

", f.affected_assets));
            }
            if !f.evidence_ids.is_empty() {
                md.push_str(&format!("### Evidence

Refs: {}

", f.evidence_ids));
            }
            if !f.cve_ids.is_empty() {
                md.push_str(&format!("### CVEs

{}

", f.cve_ids));
            }
        }
        self.report_text = md;
    }
}

fn today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let days = now.duration_since(UNIX_EPOCH).unwrap().as_secs() / 86_400;
    let (y, m, d) = unix_days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn unix_days_to_ymd(mut days: u64) -> (i32, u32, u32) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year as u64 { break; }
        days -= days_in_year as u64;
        year += 1;
    }
    let mut month = 1;
    let days_in_months = [31, if is_leap_year(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for dim in days_in_months {
        if days < dim as u64 { break; }
        days -= dim as u64;
        month += 1;
    }
    (year, month, days as u32 + 1)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

fn engagement_label(idx: usize, engagements: &[(i64, String)]) -> String {
    engagements.get(idx).map(|(_, l)| l.clone()).unwrap_or_else(|| "None".to_string())
}

pub fn draw(frame: &mut Frame, area: Rect, state: &FindingsState) {
    let rect = widgets::centered_fixed(90, area.height.min(28), area);
    let block = widgets::form_block("Findings / Report Builder");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Edit(_) => draw_form(frame, inner, state),
        Mode::View => draw_view(frame, inner, state),
        Mode::Report => draw_report(frame, inner, state),
        Mode::Export => draw_export(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &FindingsState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(area);

    let rows: Vec<Row> = state
        .findings
        .iter()
        .map(|f| Row::new(vec![f.title.clone(), f.severity.clone(), f.status.clone(), f.reported_at.clone()]))
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Min(28),
            Constraint::Length(10),
            Constraint::Length(14),
            Constraint::Length(12),
        ],
    )
    .header(Row::new(vec!["Title", "Severity", "Status", "Reported"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.findings.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "a: add  Enter: view  e: edit  d: delete  r: report  x: export findings  c: client report  Esc: back",
        theme::hint_style(),
    )];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &FindingsState) {
    let mut lines = vec![
        Line::styled(
            if matches!(state.mode, Mode::Edit(Some(_))) { "Edit Finding" } else { "New Finding" },
            theme::title_style(),
        ),
        Line::raw(""),
        field_line("Title", state.title.display(), state.focus == 0),
        field_line("Engagement", engagement_label(state.engagement_idx, &state.engagements), state.focus == 1),
        field_line("Severity", SEVERITIES[state.severity_idx].to_string(), state.focus == 2),
        field_line("Status", STATUSES[state.status_idx].to_string(), state.focus == 3),
        field_line("CVSS", state.cvss.display(), state.focus == 4),
        field_line("Affected Assets", state.affected_assets.display(), state.focus == 5),
        field_line("Evidence IDs", state.evidence_ids.display(), state.focus == 6),
        field_line("CVE IDs", state.cve_ids.display(), state.focus == 7),
        Line::raw(""),
        field_line("Description", state.description.display(), state.focus == 8),
        field_line("Remediation", state.remediation.display(), state.focus == 9),
        field_line("Reported At", state.reported_at.display(), state.focus == 10),
        Line::raw(""),
        Line::styled(
            "Tab: move  +/-: cycle  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_view(frame: &mut Frame, area: Rect, state: &FindingsState) {
    let item = state.findings.get(state.selected);
    let mut lines = vec![Line::styled("Finding Detail", theme::title_style()), Line::raw("")];
    if let Some(f) = item {
        lines.push(Line::from(vec![Span::styled("Title: ", theme::title_style()), Span::raw(f.title.clone())]));
        lines.push(Line::from(vec![Span::styled("Severity: ", theme::title_style()), Span::raw(f.severity.clone())]));
        lines.push(Line::from(vec![Span::styled("Status: ", theme::title_style()), Span::raw(f.status.clone())]));
        lines.push(Line::from(vec![Span::styled("CVSS: ", theme::title_style()), Span::raw(f.cvss.clone())]));
        lines.push(Line::from(vec![Span::styled("Reported: ", theme::title_style()), Span::raw(f.reported_at.clone())]));
        if !f.description.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled("Description:", theme::title_style()));
            for line in f.description.lines() {
                lines.push(Line::raw(line.to_string()));
            }
        }
        if !f.remediation.is_empty() {
            lines.push(Line::raw(""));
            lines.push(Line::styled("Remediation:", theme::title_style()));
            for line in f.remediation.lines() {
                lines.push(Line::raw(line.to_string()));
            }
        }
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("Esc: back  e: edit  d: delete", theme::hint_style()));
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn draw_report(frame: &mut Frame, area: Rect, state: &FindingsState) {
    let lines: Vec<Line> = state
        .report_text
        .lines()
        .map(|l| Line::raw(l.to_string()))
        .collect();
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(widgets::form_block("Report Preview")),
        area,
    );
}

fn draw_export(frame: &mut Frame, area: Rect, state: &FindingsState) {
    let label = match state.export_mode {
        ExportMode::ClientReport => "Export Client Report to File",
        _ => "Export Findings Report to File",
    };
    let mut lines = vec![
        Line::styled(label, theme::title_style()),
        Line::raw(""),
        field_line("Path", state.export_path.display(), true),
        Line::raw(""),
        Line::styled("Enter: write  Esc: cancel", theme::hint_style()),
    ];
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn save_finding(state: &mut FindingsState) {
    if state.title.value.trim().is_empty() {
        state.status = "Title is required.".to_string();
        return;
    }
    let engagement_id = state.engagements.get(state.engagement_idx).map(|(id, _)| *id).unwrap_or(0);
    let severity = SEVERITIES[state.severity_idx].to_string();
    let status = STATUSES[state.status_idx].to_string();
    let result = match state.mode {
        Mode::Edit(Some(id)) => findings::update_finding(
            state.user_id,
            id,
            engagement_id,
            state.title.value.trim(),
            &severity,
            &status,
            &state.cvss.value,
            &state.description.value,
            &state.remediation.value,
            &state.affected_assets.value,
            &state.evidence_ids.value,
            &state.cve_ids.value,
            &state.reported_at.value,
        )
        .map(|_| ()),
        _ => findings::add_finding(
            state.user_id,
            engagement_id,
            state.title.value.trim(),
            &severity,
            &status,
            &state.cvss.value,
            &state.description.value,
            &state.remediation.value,
            &state.affected_assets.value,
            &state.evidence_ids.value,
            &state.cve_ids.value,
            &state.reported_at.value,
        )
        .map(|_| ()),
    };
    match result {
        Ok(_) => {
            state.status = "Saved.".to_string();
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.status = format!("Error saving finding: {e}"),
    }
}

fn do_export(state: &mut FindingsState) {
    let path = state.export_path.value.trim();
    if path.is_empty() {
        state.status = "Path is required.".to_string();
        return;
    }
    let content = match state.export_mode {
        ExportMode::ClientReport => state.report_text.clone(),
        _ => state.report_text.clone(),
    };
    match std::fs::write(path, content) {
        Ok(()) => {
            state.status = format!("Report written to {path}");
            state.mode = Mode::Report;
            state.export_mode = ExportMode::None;
        }
        Err(e) => state.status = format!("Export failed: {e}"),
    }
}

fn handle_list_key(state: &mut FindingsState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.findings.is_empty() {
                state.selected = if state.selected == 0 { state.findings.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.findings.is_empty() {
                state.selected = (state.selected + 1) % state.findings.len();
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
            state.load_selected_into_form();
            state.mode = Mode::Edit(Some(state.findings.get(state.selected).map(|f| f.id).unwrap_or(0)));
            state.status.clear();
            Action::None
        }
        KeyCode::Enter => {
            if !state.findings.is_empty() {
                state.mode = Mode::View;
                state.status.clear();
            }
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(f) = state.findings.get(state.selected) {
                let id = f.id;
                match findings::delete_finding(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Finding deleted.".to_string();
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting finding: {e}"),
                }
            }
            Action::None
        }
        KeyCode::Char('r') => {
            state.build_report();
            state.mode = Mode::Report;
            Action::None
        }
        KeyCode::Char('x') => {
            state.export_mode = ExportMode::FindingsOnly;
            state.build_report();
            state.export_path = widgets::TextField::with_value(format!("{}/findings-report.md", std::env::var("HOME").unwrap_or_default()));
            state.mode = Mode::Export;
            Action::None
        }
        KeyCode::Char('c') => {
            if state.engagements.is_empty() {
                state.status = "No engagements to base client report on.".to_string();
                Action::None
            } else {
                let engagement_id = state.engagements[state.engagement_idx.min(state.engagements.len() - 1)].0;
                match reports::build_client_report(state.user_id, None, engagement_id) {
                    Ok(md) => {
                        state.report_text = md;
                        state.export_mode = ExportMode::ClientReport;
                        state.export_path = widgets::TextField::with_value(format!("{}/client-report-{}.md", std::env::var("HOME").unwrap_or_default(), engagement_id));
                        state.mode = Mode::Export;
                    }
                    Err(e) => state.status = format!("Report error: {e}"),
                }
                Action::None
            }
        }
        _ => Action::None,
    }
}

fn handle_edit_key(state: &mut FindingsState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            Action::None
        }
        KeyCode::Tab | KeyCode::Down => {
            state.focus = (state.focus + 1) % (FIELD_COUNT + 3);
            Action::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            state.focus = (state.focus + FIELD_COUNT + 2) % (FIELD_COUNT + 3);
            Action::None
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            match state.focus {
                1 => state.engagement_idx = (state.engagement_idx + 1) % state.engagements.len().max(1),
                2 => state.severity_idx = (state.severity_idx + 1) % SEVERITIES.len(),
                3 => state.status_idx = (state.status_idx + 1) % STATUSES.len(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char('-') => {
            match state.focus {
                1 => state.engagement_idx = (state.engagement_idx + state.engagements.len().max(1) - 1) % state.engagements.len().max(1),
                2 => state.severity_idx = (state.severity_idx + SEVERITIES.len() - 1) % SEVERITIES.len(),
                3 => state.status_idx = (state.status_idx + STATUSES.len() - 1) % STATUSES.len(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Enter => {
            save_finding(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.title.backspace(),
                4 => state.cvss.backspace(),
                5 => state.affected_assets.backspace(),
                6 => state.evidence_ids.backspace(),
                7 => state.cve_ids.backspace(),
                8 => state.description.backspace(),
                9 => state.remediation.backspace(),
                10 => state.reported_at.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.title.push_char(c),
                4 => state.cvss.push_char(c),
                5 => state.affected_assets.push_char(c),
                6 => state.evidence_ids.push_char(c),
                7 => state.cve_ids.push_char(c),
                8 => state.description.push_char(c),
                9 => state.remediation.push_char(c),
                10 => state.reported_at.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_view_key(state: &mut FindingsState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            Action::None
        }
        KeyCode::Char('e') | KeyCode::Enter => {
            state.load_selected_into_form();
            state.mode = Mode::Edit(Some(state.findings.get(state.selected).map(|f| f.id).unwrap_or(0)));
            state.status.clear();
            Action::None
        }
        KeyCode::Char('d') => {
            if let Some(f) = state.findings.get(state.selected) {
                let id = f.id;
                match findings::delete_finding(state.user_id, id) {
                    Ok(()) => {
                        state.status = "Finding deleted.".to_string();
                        state.mode = Mode::List;
                        state.refresh();
                    }
                    Err(e) => state.status = format!("Error deleting finding: {e}"),
                }
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut FindingsState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::Edit(_) => handle_edit_key(state, key),
        Mode::View => handle_view_key(state, key),
        Mode::Report => {
            if key.code == KeyCode::Esc || key.code == KeyCode::Char('r') {
                state.mode = Mode::List;
            }
            Action::None
        }
        Mode::Export => {
            match key.code {
                KeyCode::Esc => {
                    state.mode = Mode::Report;
                    Action::None
                }
                KeyCode::Enter => {
                    do_export(state);
                    Action::None
                }
                KeyCode::Backspace => {
                    state.export_path.backspace();
                    Action::None
                }
                KeyCode::Char(c) => {
                    state.export_path.push_char(c);
                    Action::None
                }
                _ => Action::None,
            }
        }
    }
}
