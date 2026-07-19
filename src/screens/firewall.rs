use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::ui::{theme, widgets};

pub const DIRECTIONS: &[&str] = &["Inbound", "Outbound"];
pub const ACTIONS: &[&str] = &["Allow", "Block"];
pub const PROTOCOLS: &[&str] = &["TCP", "UDP", "Any"];

const FIELD_COUNT: usize = 5;

pub struct FirewallRule {
    pub name: String,
    pub display_name: String,
    pub direction: String,
    pub action: String,
    pub enabled: String,
}

enum Mode {
    List,
    Add,
}

pub struct FirewallState {
    rules: Vec<FirewallRule>,
    selected: usize,
    mode: Mode,
    name: widgets::TextField,
    direction_idx: usize,
    action_idx: usize,
    protocol_idx: usize,
    port: widgets::TextField,
    focus: usize,
    status: String,
    error: Option<String>,
}

impl FirewallState {
    pub fn new() -> Self {
        let mut state = FirewallState {
            rules: Vec::new(),
            selected: 0,
            mode: Mode::List,
            name: widgets::TextField::new(),
            direction_idx: 0,
            action_idx: 0,
            protocol_idx: 0,
            port: widgets::TextField::new(),
            focus: 0,
            status: String::new(),
            error: None,
        };
        state.refresh();
        state
    }

    fn refresh(&mut self) {
        self.error = None;
        match query_rules() {
            Ok(rules) => {
                self.rules = rules;
                if self.selected >= self.rules.len() {
                    self.selected = self.rules.len().saturating_sub(1);
                }
                self.status = "Refreshed.".to_string();
            }
            Err(e) => {
                self.rules.clear();
                self.error = Some(format!("Rules: {e}"));
            }
        }
    }

    fn clear_form(&mut self) {
        self.name = widgets::TextField::new();
        self.direction_idx = 0;
        self.action_idx = 0;
        self.protocol_idx = 0;
        self.port = widgets::TextField::new();
        self.focus = 0;
    }
}

// ── Windows: PowerShell NetFirewall ─────────────────────────────

#[cfg(windows)]
fn run_powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Operation failed — this likely requires running Tsukuyomi OS as Administrator.".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(windows)]
fn query_rules() -> Result<Vec<FirewallRule>, String> {
    let script = "Get-NetFirewallRule | ForEach-Object { \"$($_.Name)|$($_.DisplayName)|$($_.Direction)|$($_.Action)|$($_.Enabled)\" }";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "Command failed.".to_string() } else { stderr });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut rules = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(5, '|');
        rules.push(FirewallRule {
            name: parts.next().unwrap_or("").to_string(),
            display_name: parts.next().unwrap_or("").to_string(),
            direction: parts.next().unwrap_or("").to_string(),
            action: parts.next().unwrap_or("").to_string(),
            enabled: parts.next().unwrap_or("").to_string(),
        });
    }
    Ok(rules)
}

#[cfg(windows)]
fn set_rule_enabled(name: &str, enable: bool) -> Result<(), String> {
    let escaped = name.replace('\'', "''");
    let cmd = if enable {
        format!("Enable-NetFirewallRule -Name '{escaped}' -ErrorAction Stop")
    } else {
        format!("Disable-NetFirewallRule -Name '{escaped}' -ErrorAction Stop")
    };
    run_powershell(&cmd).map(|_| ())
}

#[cfg(windows)]
fn add_rule(name: &str, direction: &str, action: &str, protocol: &str, port: &str) -> Result<(), String> {
    let escaped_name = name.replace('\'', "''");
    let mut cmd = format!(
        "New-NetFirewallRule -DisplayName '{escaped_name}' -Direction {direction} -Action {action} -ErrorAction Stop"
    );
    if protocol != "Any" {
        cmd.push_str(&format!(" -Protocol {protocol}"));
        let port = port.trim();
        if !port.is_empty() {
            let escaped_port = port.replace('\'', "''");
            cmd.push_str(&format!(" -LocalPort {escaped_port}"));
        }
    }
    run_powershell(&cmd).map(|_| ())
}

// ── Linux: ufw / iptables ───────────────────────────────────────

#[cfg(unix)]
fn run_cmd(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Operation failed — this likely requires running Tsukuyomi OS as root/sudo.".to_string()
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(unix)]
fn has_ufw() -> bool {
    Command::new("which")
        .arg("ufw")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn query_rules() -> Result<Vec<FirewallRule>, String> {
    if has_ufw() {
        let output = Command::new("ufw")
            .args(["status", "verbose"])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            // ufw may require root — try with sudo
            let output = Command::new("sudo")
                .args(["ufw", "status", "verbose"])
                .output()
                .map_err(|e| e.to_string())?;
            if !output.status.success() {
                return Err("ufw status failed — run as root/sudo".to_string());
            }
            return parse_ufw_rules(&String::from_utf8_lossy(&output.stdout));
        }
        parse_ufw_rules(&String::from_utf8_lossy(&output.stdout))
    } else {
        // Fall back to iptables -L
        let output = Command::new("iptables")
            .args(["-L", "--line-numbers", "-n"])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("Neither ufw nor iptables accessible — install ufw or run as root".to_string());
        }
        parse_iptables_rules(&String::from_utf8_lossy(&output.stdout))
    }
}

#[cfg(unix)]
fn parse_ufw_rules(stdout: &str) -> Result<Vec<FirewallRule>, String> {
    let mut rules = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        // Skip headers and blank lines
        if line.is_empty() || line.starts_with("Status:") || line.starts_with("Logging") || line.contains("Default:") {
            continue;
        }
        // Parse lines like: "22/tcp                     ALLOW IN    Anywhere"
        // or "80/tcp (v6)                DENY IN     Anywhere (v6)"
        if line.contains("ALLOW") || line.contains("DENY") || line.contains("LIMIT") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let port_proto = parts.get(0).unwrap_or(&"").to_string();
                let action_str = parts.get(1).unwrap_or(&"").to_string();
                let direction = if line.contains("IN") { "Inbound" }
                    else if line.contains("OUT") { "Outbound" }
                    else { "Inbound" };
                let action = if action_str.contains("ALLOW") || action_str.contains("LIMIT") { "Allow" }
                    else { "Block" };
                let enabled = if line.contains("(deny)") || line.contains("(reject)") { "False" }
                    else { "True" };
                rules.push(FirewallRule {
                    name: port_proto.clone(),
                    display_name: port_proto,
                    direction: direction.to_string(),
                    action: action.to_string(),
                    enabled: enabled.to_string(),
                });
            }
        }
    }
    Ok(rules)
}

#[cfg(unix)]
fn parse_iptables_rules(stdout: &str) -> Result<Vec<FirewallRule>, String> {
    let mut rules = Vec::new();
    let mut current_chain = String::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("Chain") {
            current_chain = line.split_whitespace().nth(1).unwrap_or("").to_string();
            continue;
        }
        if line.is_empty() || line.starts_with("num") || line.starts_with("target") {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            let target = parts.get(1).unwrap_or(&"").to_string();
            let action = if target == "ACCEPT" { "Allow" } else if target == "DROP" || target == "REJECT" { "Block" } else { "Allow" };
            let direction = match current_chain.as_str() {
                "INPUT" => "Inbound",
                "OUTPUT" => "Outbound",
                "FORWARD" => "Forward",
                _ => "Inbound",
            };
            rules.push(FirewallRule {
                name: format!("{}-{}", current_chain, parts.get(0).unwrap_or(&"?")),
                display_name: format!("{} rule #{}", current_chain, parts.get(0).unwrap_or(&"?")),
                direction: direction.to_string(),
                action: action.to_string(),
                enabled: "True".to_string(),
            });
        }
    }
    Ok(rules)
}

#[cfg(unix)]
fn set_rule_enabled(name: &str, enable: bool) -> Result<(), String> {
    // UFW: delete + re-add or toggle. For simplicity, we use ufw delete
    if has_ufw() {
        let action = if enable { "enable" } else { "disable" };
        // Can't easily toggle individual rules in ufw — would need to delete/recreate
        Err(format!("Toggling individual rules is not supported with ufw. Use 'ufw {action}' to toggle the firewall itself."))
    } else {
        Err("Toggling individual rules requires ufw. Install with: apt install ufw".to_string())
    }
}

#[cfg(unix)]
fn add_rule(name: &str, direction: &str, action: &str, protocol: &str, port: &str) -> Result<(), String> {
    if !has_ufw() {
        return Err("ufw is required to add firewall rules. Install with: apt install ufw".to_string());
    }

    let port_val = port.trim();
    let proto_lower = protocol.to_lowercase();

    // Build ufw command: ufw allow/deny in/out proto port
    let ufw_action = if action == "Allow" { "allow" } else { "deny" };
    let ufw_dir = if direction == "Inbound" { "in" } else { "out" };

    let mut args: Vec<String> = vec![ufw_action.to_string(), ufw_dir.to_string()];

    if protocol != "Any" && !port_val.is_empty() {
        args.push("proto".to_string());
        args.push(proto_lower.to_string());
        args.push("to".to_string());
        args.push("any".to_string());
        args.push("port".to_string());
        args.push(port_val.to_string());
    } else if !port_val.is_empty() {
        args.push(port_val.to_string());
    }

    // Use sudo if not root
    let (cmd, prefix): (&str, Vec<String>) = if std::env::var("EUID").map(|e| e == "0").unwrap_or(false) {
        ("ufw", args)
    } else {
        let mut sudo_args = vec!["-n".to_string(), "ufw".to_string()];
        sudo_args.extend(args);
        ("sudo", sudo_args)
    };

    let output = Command::new(cmd)
        .args(&prefix)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            format!("ufw rule creation failed — may require sudo/root access")
        } else {
            stderr
        });
    }
    Ok(())
}

// ── Cross-platform dispatch ─────────────────────────────────────

fn toggle_selected(state: &mut FirewallState, enable: bool) {
    let Some(rule) = state.rules.get(state.selected) else { return };
    let name = rule.name.clone();
    let display_name = rule.display_name.clone();
    match set_rule_enabled(&name, enable) {
        Ok(()) => {
            state.error = None;
            state.refresh();
            state.status = format!("{display_name}: {} succeeded.", if enable { "enable" } else { "disable" });
        }
        Err(e) => {
            state.error = Some(format!("{display_name}: {} failed — {e}", if enable { "enable" } else { "disable" }));
        }
    }
}

fn save_new_rule(state: &mut FirewallState) {
    if state.name.value.trim().is_empty() {
        state.status = "Name is required.".to_string();
        return;
    }
    let direction = DIRECTIONS[state.direction_idx];
    let action = ACTIONS[state.action_idx];
    let protocol = PROTOCOLS[state.protocol_idx];
    match add_rule(state.name.value.trim(), direction, action, protocol, &state.port.value) {
        Ok(()) => {
            state.status = "Rule created.".to_string();
            state.error = None;
            state.mode = Mode::List;
            state.refresh();
        }
        Err(e) => state.error = Some(format!("Add rule failed — {e}")),
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &FirewallState) {
    let rect = widgets::centered_fixed(100, area.height.min(32), area);
    let block = widgets::form_block("Firewall Rule Manager");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    match state.mode {
        Mode::List => draw_list(frame, inner, state),
        Mode::Add => draw_form(frame, inner, state),
    }
}

fn draw_list(frame: &mut Frame, area: Rect, state: &FirewallState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(area);

    let rows: Vec<Row> = state
        .rules
        .iter()
        .map(|r| {
            Row::new(vec![
                r.display_name.clone(),
                r.direction.clone(),
                r.action.clone(),
                r.enabled.clone(),
            ])
        })
        .collect();
    let table = Table::new(
        rows,
        [
            Constraint::Min(40),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(Row::new(vec!["Display Name", "Direction", "Action", "Enabled"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block(""));
    let mut table_state = TableState::default()
        .with_selected(if state.rules.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(table, chunks[0], &mut table_state);

    let mut lines = vec![Line::styled(
        "Up/Down: select  g: enable  x: disable  a: add rule  r: refresh  Esc: back",
        theme::hint_style(),
    )];
    if let Some(err) = &state.error {
        lines.push(Line::styled(err.clone(), theme::error_style()));
    }
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[1]);
}

fn draw_form(frame: &mut Frame, area: Rect, state: &FirewallState) {
    let mut lines = vec![
        Line::styled("New Firewall Rule", theme::title_style()),
        Line::raw(""),
        field_line("Name", state.name.display(), state.focus == 0),
        field_line("Direction", DIRECTIONS[state.direction_idx].to_string(), state.focus == 1),
        field_line("Action", ACTIONS[state.action_idx].to_string(), state.focus == 2),
        field_line("Protocol", PROTOCOLS[state.protocol_idx].to_string(), state.focus == 3),
        field_line("Local Port", state.port.display(), state.focus == 4),
        Line::raw(""),
        Line::styled(
            "Tab: move  Left/Right: change  Enter: save  Esc: cancel",
            theme::hint_style(),
        ),
    ];
    if let Some(err) = &state.error {
        lines.push(Line::styled(err.clone(), theme::error_style()));
    }
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn handle_list_key(state: &mut FirewallState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Up => {
            if !state.rules.is_empty() {
                state.selected = if state.selected == 0 { state.rules.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.rules.is_empty() {
                state.selected = (state.selected + 1) % state.rules.len();
            }
            Action::None
        }
        KeyCode::Char('a') => {
            state.clear_form();
            state.mode = Mode::Add;
            state.status.clear();
            state.error = None;
            Action::None
        }
        KeyCode::Char('g') => {
            toggle_selected(state, true);
            Action::None
        }
        KeyCode::Char('x') => {
            toggle_selected(state, false);
            Action::None
        }
        KeyCode::Char('r') => {
            state.refresh();
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_add_key(state: &mut FirewallState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.mode = Mode::List;
            state.status.clear();
            state.error = None;
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
        KeyCode::Left if state.focus == 1 => {
            state.direction_idx = (state.direction_idx + DIRECTIONS.len() - 1) % DIRECTIONS.len();
            Action::None
        }
        KeyCode::Right if state.focus == 1 => {
            state.direction_idx = (state.direction_idx + 1) % DIRECTIONS.len();
            Action::None
        }
        KeyCode::Left if state.focus == 2 => {
            state.action_idx = (state.action_idx + ACTIONS.len() - 1) % ACTIONS.len();
            Action::None
        }
        KeyCode::Right if state.focus == 2 => {
            state.action_idx = (state.action_idx + 1) % ACTIONS.len();
            Action::None
        }
        KeyCode::Left if state.focus == 3 => {
            state.protocol_idx = (state.protocol_idx + PROTOCOLS.len() - 1) % PROTOCOLS.len();
            Action::None
        }
        KeyCode::Right if state.focus == 3 => {
            state.protocol_idx = (state.protocol_idx + 1) % PROTOCOLS.len();
            Action::None
        }
        KeyCode::Enter => {
            save_new_rule(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.name.backspace(),
                4 => state.port.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.name.push_char(c),
                4 => state.port.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}

pub fn handle_key(state: &mut FirewallState, key: KeyEvent) -> Action {
    match state.mode {
        Mode::List => handle_list_key(state, key),
        Mode::Add => handle_add_key(state, key),
    }
}