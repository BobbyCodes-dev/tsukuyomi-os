use std::process::Command;
use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::ui::{theme, widgets};

const COMMON_PORTS: [u16; 8] = [21, 22, 23, 25, 53, 80, 443, 3389];

enum DiagKind {
    Ping,
    Traceroute,
    Ports,
}

pub struct NetworkState {
    host: widgets::TextField,
    ports: widgets::TextField,
    focus: usize,
    output: Vec<String>,
    running: bool,
    rx: Option<Receiver<(DiagKind, Result<Vec<String>, String>)>>,
    status: String,
}

impl NetworkState {
    pub fn new() -> Self {
        NetworkState {
            host: widgets::TextField::new(),
            ports: widgets::TextField::new(),
            focus: 0,
            output: Vec::new(),
            running: false,
            rx: None,
            status: "Enter a host, then F1 ping / F2 traceroute / F3 port check / F4 interfaces."
                .to_string(),
        }
    }

    pub fn poll_diag(&mut self) {
        let Some(rx) = &self.rx else { return };
        match rx.try_recv() {
            Ok((kind, result)) => {
                let label = match kind {
                    DiagKind::Ping => "Ping",
                    DiagKind::Traceroute => "Traceroute",
                    DiagKind::Ports => "Port check",
                };
                match result {
                    Ok(lines) => {
                        self.output = lines;
                        self.status = format!("{label} complete.");
                    }
                    Err(e) => {
                        self.output = vec![format!("Error: {e}")];
                        self.status = format!("{label} failed.");
                    }
                }
                self.running = false;
                self.rx = None;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.running = false;
                self.rx = None;
            }
        }
    }
}

fn ping_host(host: &str) -> Result<Vec<String>, String> {
    let output = Command::new("ping").args(["-n", "4", host]).output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "No output from ping.".to_string() } else { stderr });
    }
    Ok(stdout.lines().map(|l| l.to_string()).collect())
}

fn traceroute_host(host: &str) -> Result<Vec<String>, String> {
    let output = Command::new("tracert")
        .args(["-d", "-h", "20", host])
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "No output from tracert.".to_string() } else { stderr });
    }
    Ok(stdout.lines().map(|l| l.to_string()).collect())
}

fn parse_ports(input: &str) -> Vec<u16> {
    let parsed: Vec<u16> = input
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|s| s.trim().parse::<u16>().ok())
        .collect();
    if parsed.is_empty() {
        COMMON_PORTS.to_vec()
    } else {
        parsed
    }
}

fn check_ports(host: &str, ports: &[u16]) -> Result<Vec<String>, String> {
    let escaped = host.replace('\'', "''");
    let mut results = Vec::new();
    for port in ports {
        let cmd = format!(
            "(Test-NetConnection -ComputerName '{escaped}' -Port {port} -WarningAction SilentlyContinue).TcpTestSucceeded"
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &cmd])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            results.push(format!(
                "Port {port}: error — {}",
                if stderr.is_empty() { "check failed".to_string() } else { stderr }
            ));
            continue;
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let status = if text.eq_ignore_ascii_case("true") { "OPEN" } else { "CLOSED/FILTERED" };
        results.push(format!("Port {port}: {status}"));
    }
    Ok(results)
}

fn query_interfaces() -> Result<Vec<String>, String> {
    let script = "Get-NetAdapter | Where-Object { $_.Status -eq 'Up' } | ForEach-Object { $stats = Get-NetAdapterStatistics -Name $_.Name; \"$($_.Name)|$($_.LinkSpeed)|$($stats.ReceivedBytes)|$($stats.SentBytes)\" }";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "Command failed.".to_string() } else { stderr });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(4, '|');
        let name = parts.next().unwrap_or("");
        let speed = parts.next().unwrap_or("");
        let rx: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let tx: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
        lines.push(format!(
            "{name} ({speed}): RX {:.2} MB, TX {:.2} MB",
            rx as f64 / 1_048_576.0,
            tx as f64 / 1_048_576.0
        ));
    }
    Ok(lines)
}

fn start_ping(state: &mut NetworkState) {
    if state.running {
        state.status = "A check is already in progress.".to_string();
        return;
    }
    let host = state.host.value.trim().to_string();
    if host.is_empty() {
        state.status = "Enter a host first.".to_string();
        return;
    }
    state.status = format!("Pinging {host}...");
    state.output.clear();
    state.running = true;
    let (tx, rx) = mpsc::channel();
    state.rx = Some(rx);
    std::thread::spawn(move || {
        let result = ping_host(&host);
        let _ = tx.send((DiagKind::Ping, result));
    });
}

fn start_traceroute(state: &mut NetworkState) {
    if state.running {
        state.status = "A check is already in progress.".to_string();
        return;
    }
    let host = state.host.value.trim().to_string();
    if host.is_empty() {
        state.status = "Enter a host first.".to_string();
        return;
    }
    state.status = format!("Tracing route to {host}...");
    state.output.clear();
    state.running = true;
    let (tx, rx) = mpsc::channel();
    state.rx = Some(rx);
    std::thread::spawn(move || {
        let result = traceroute_host(&host);
        let _ = tx.send((DiagKind::Traceroute, result));
    });
}

fn start_port_check(state: &mut NetworkState) {
    if state.running {
        state.status = "A check is already in progress.".to_string();
        return;
    }
    let host = state.host.value.trim().to_string();
    if host.is_empty() {
        state.status = "Enter a host first.".to_string();
        return;
    }
    let ports = parse_ports(&state.ports.value);
    state.status = format!("Checking {} port(s) on {host}...", ports.len());
    state.output.clear();
    state.running = true;
    let (tx, rx) = mpsc::channel();
    state.rx = Some(rx);
    std::thread::spawn(move || {
        let result = check_ports(&host, &ports);
        let _ = tx.send((DiagKind::Ports, result));
    });
}

fn refresh_interfaces(state: &mut NetworkState) {
    if state.running {
        state.status = "A check is already in progress.".to_string();
        return;
    }
    state.output.clear();
    match query_interfaces() {
        Ok(lines) => {
            if lines.is_empty() {
                state.output.push("No active network adapters found.".to_string());
            } else {
                state.output = lines;
            }
            state.status = "Interface stats refreshed.".to_string();
        }
        Err(e) => {
            state.output = vec![format!("Error: {e}")];
            state.status = "Failed to query interfaces.".to_string();
        }
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &NetworkState) {
    let rect = widgets::centered_fixed(96, area.height.min(32), area);
    let block = widgets::form_block("Network Diagnostics");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(6),
            Constraint::Length(2),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(field_line("Host", state.host.display(), state.focus == 0)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(field_line(
            "Ports (comma-separated, blank = common)",
            state.ports.display(),
            state.focus == 1,
        )),
        chunks[1],
    );
    frame.render_widget(Paragraph::new(Line::styled(state.status.clone(), theme::clock_style())), chunks[2]);

    let visible = chunks[3].height.saturating_sub(2) as usize;
    let all: Vec<Line> = state.output.iter().map(|l| Line::raw(l.clone())).collect();
    let start = all.len().saturating_sub(visible);
    let output_widget = Paragraph::new(all[start..].to_vec())
        .block(widgets::log_block("Output"))
        .wrap(Wrap { trim: false });
    frame.render_widget(output_widget, chunks[3]);

    frame.render_widget(
        Paragraph::new(Line::styled(
            "Tab: switch field  F1: ping  F2: traceroute  F3: port check  F4: interfaces  Esc: back",
            theme::hint_style(),
        ))
        .wrap(Wrap { trim: false }),
        chunks[4],
    );
}

pub fn handle_key(state: &mut NetworkState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Tab | KeyCode::BackTab => {
            state.focus = (state.focus + 1) % 2;
            Action::None
        }
        KeyCode::F(1) => {
            start_ping(state);
            Action::None
        }
        KeyCode::F(2) => {
            start_traceroute(state);
            Action::None
        }
        KeyCode::F(3) => {
            start_port_check(state);
            Action::None
        }
        KeyCode::F(4) => {
            refresh_interfaces(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.host.backspace(),
                _ => state.ports.backspace(),
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.host.push_char(c),
                _ => state.ports.push_char(c),
            }
            Action::None
        }
        _ => Action::None,
    }
}
