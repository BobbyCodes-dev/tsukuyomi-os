use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, TableState, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::ui::{theme, widgets};

pub struct DiskStat {
    pub drive: String,
    pub free_gb: f64,
    pub total_gb: f64,
}

pub struct ServiceStat {
    pub name: String,
    pub display_name: String,
    pub status: String,
}

pub struct HealthState {
    cpu_percent: Option<f64>,
    ram_used_mb: u64,
    ram_total_mb: u64,
    disks: Vec<DiskStat>,
    services: Vec<ServiceStat>,
    selected: usize,
    error: Option<String>,
    status: String,
}

impl HealthState {
    pub fn new() -> Self {
        let mut state = HealthState {
            cpu_percent: None,
            ram_used_mb: 0,
            ram_total_mb: 0,
            disks: Vec::new(),
            services: Vec::new(),
            selected: 0,
            error: None,
            status: String::new(),
        };
        state.refresh();
        state
    }

    pub fn refresh(&mut self) {
        self.error = None;
        self.status.clear();
        let mut errors = Vec::new();

        match query_cpu() {
            Ok(pct) => self.cpu_percent = Some(pct),
            Err(e) => {
                self.cpu_percent = None;
                errors.push(format!("CPU: {e}"));
            }
        }

        match query_ram() {
            Ok((used_mb, total_mb)) => {
                self.ram_total_mb = total_mb;
                self.ram_used_mb = used_mb;
            }
            Err(e) => {
                self.ram_total_mb = 0;
                self.ram_used_mb = 0;
                errors.push(format!("RAM: {e}"));
            }
        }

        match query_disks() {
            Ok(disks) => self.disks = disks,
            Err(e) => {
                self.disks.clear();
                errors.push(format!("Disks: {e}"));
            }
        }

        match query_services() {
            Ok(services) => {
                self.services = services;
                if self.selected >= self.services.len() {
                    self.selected = self.services.len().saturating_sub(1);
                }
            }
            Err(e) => {
                self.services.clear();
                errors.push(format!("Services: {e}"));
            }
        }

        if !errors.is_empty() {
            self.error = Some(errors.join("  |  "));
        } else {
            self.status = "Refreshed.".to_string();
        }
    }
}

// ── Windows: PowerShell / CIM ───────────────────────────────────

#[cfg(windows)]
fn run_powershell(script: &str) -> Result<String, String> {
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() { "Command failed.".to_string() } else { stderr });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(windows)]
fn query_cpu() -> Result<f64, String> {
    let text = run_powershell(
        "(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average",
    )?;
    text.parse::<f64>().map_err(|_| format!("Unexpected CPU output: {text}"))
}

#[cfg(windows)]
fn query_ram() -> Result<(u64, u64), String> {
    let text = run_powershell(
        "Get-CimInstance Win32_OperatingSystem | ForEach-Object { \"$($_.FreePhysicalMemory)|$($_.TotalVisibleMemorySize)\" }",
    )?;
    let mut parts = text.splitn(2, '|');
    let free_kb: u64 = parts.next().unwrap_or("").parse().map_err(|_| format!("Unexpected RAM output: {text}"))?;
    let total_kb: u64 = parts.next().unwrap_or("").parse().map_err(|_| format!("Unexpected RAM output: {text}"))?;
    let used_mb = total_kb.saturating_sub(free_kb) / 1024;
    let total_mb = total_kb / 1024;
    Ok((used_mb, total_mb))
}

#[cfg(windows)]
fn query_disks() -> Result<Vec<DiskStat>, String> {
    let script = "Get-CimInstance Win32_LogicalDisk -Filter \"DriveType=3\" | ForEach-Object { \"$($_.DeviceID)|$($_.FreeSpace)|$($_.Size)\" }";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Command failed.".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(3, '|');
        let drive = parts.next().unwrap_or("").to_string();
        let free: f64 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
        let total: f64 = parts.next().unwrap_or("0").parse().unwrap_or(0.0);
        disks.push(DiskStat { drive, free_gb: free / 1_073_741_824.0, total_gb: total / 1_073_741_824.0 });
    }
    Ok(disks)
}

#[cfg(windows)]
fn query_services() -> Result<Vec<ServiceStat>, String> {
    let script = "Get-Service | ForEach-Object { \"$($_.Name)|$($_.DisplayName)|$($_.Status)\" }";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("Command failed.".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let mut parts = line.splitn(3, '|');
        services.push(ServiceStat {
            name: parts.next().unwrap_or("").to_string(),
            display_name: parts.next().unwrap_or("").to_string(),
            status: parts.next().unwrap_or("").to_string(),
        });
    }
    Ok(services)
}

#[cfg(windows)]
fn control_service(name: &str, action: &str) -> Result<(), String> {
    let escaped = name.replace('\'', "''");
    let cmd = match action {
        "start" => format!("Start-Service -Name '{escaped}' -ErrorAction Stop"),
        "stop" => format!("Stop-Service -Name '{escaped}' -ErrorAction Stop"),
        _ => format!("Restart-Service -Name '{escaped}' -ErrorAction Stop"),
    };
    let output = Command::new("powershell")
        .args(["-NoProfile", "-Command", &cmd])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "Operation failed — this likely requires running Tsukuyomi OS as Administrator.".to_string()
        } else { stderr });
    }
    Ok(())
}

// ── Linux: /proc + systemctl + df ───────────────────────────────

#[cfg(unix)]
fn query_cpu() -> Result<f64, String> {
    // Read /proc/stat for CPU usage
    let stat1 = std::fs::read_to_string("/proc/stat").map_err(|e| e.to_string())?;
    let line1 = stat1.lines().next().ok_or("Empty /proc/stat")?;
    let vals1: Vec<u64> = line1.split_whitespace().skip(1).filter_map(|s| s.parse().ok()).collect();
    if vals1.len() < 4 { return Err("Invalid /proc/stat format".to_string()); }
    let idle1 = vals1[3];
    let total1: u64 = vals1.iter().sum();

    std::thread::sleep(std::time::Duration::from_millis(100));

    let stat2 = std::fs::read_to_string("/proc/stat").map_err(|e| e.to_string())?;
    let line2 = stat2.lines().next().ok_or("Empty /proc/stat")?;
    let vals2: Vec<u64> = line2.split_whitespace().skip(1).filter_map(|s| s.parse().ok()).collect();
    if vals2.len() < 4 { return Err("Invalid /proc/stat format".to_string()); }
    let idle2 = vals2[3];
    let total2: u64 = vals2.iter().sum();

    let total_delta = total2.saturating_sub(total1) as f64;
    let idle_delta = idle2.saturating_sub(idle1) as f64;
    if total_delta == 0.0 { return Ok(0.0); }
    Ok(((total_delta - idle_delta) / total_delta) * 100.0)
}

#[cfg(unix)]
fn query_ram() -> Result<(u64, u64), String> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").map_err(|e| e.to_string())?;
    let mut mem_total_kb: u64 = 0;
    let mut mem_available_kb: u64 = 0;

    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            mem_total_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            mem_available_kb = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        }
    }

    if mem_total_kb == 0 { return Err("Could not parse /proc/meminfo".to_string()); }

    let total_mb = mem_total_kb / 1024;
    let used_mb = mem_total_kb.saturating_sub(mem_available_kb) / 1024;
    Ok((used_mb, total_mb))
}

#[cfg(unix)]
fn query_disks() -> Result<Vec<DiskStat>, String> {
    let output = Command::new("df")
        .args(["-B1", "--output=source,size,avail,target"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("df command failed".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut disks = Vec::new();
    for line in stdout.lines().skip(1) { // skip header
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }
        let source = parts[0];
        // Skip tmpfs, devtmpfs, overlay, etc.
        if source.starts_with("tmpfs") || source.starts_with("dev") || source == "overlay" || source == "none" {
            continue;
        }
        let total: f64 = parts[1].parse().unwrap_or(0.0);
        let avail: f64 = parts[2].parse().unwrap_or(0.0);
        let mount = parts[3];
        disks.push(DiskStat {
            drive: mount.to_string(),
            free_gb: avail / 1_073_741_824.0,
            total_gb: total / 1_073_741_824.0,
        });
    }
    Ok(disks)
}

#[cfg(unix)]
fn query_services() -> Result<Vec<ServiceStat>, String> {
    // Use systemctl for systemd-based systems
    let output = Command::new("systemctl")
        .args(["list-units", "--type=service", "--no-legend", "--no-pager"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("systemctl not available".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut services = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 { continue; }
        let name = parts[0].trim_end_matches(".service").to_string();
        let status = if parts[3] == "running" { "Running" } else { "Stopped" }.to_string();
        // Display name = unit name without .service suffix
        let display_name = name.clone();
        services.push(ServiceStat { name, display_name, status });
    }
    Ok(services)
}

#[cfg(unix)]
fn control_service(name: &str, action: &str) -> Result<(), String> {
    let unit = format!("{name}.service");
    let systemctl_action = match action {
        "start" => "start",
        "stop" => "stop",
        _ => "restart",
    };

    // Try without sudo first, fall back to sudo
    let output = Command::new("systemctl")
        .args([systemctl_action, &unit])
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        // Try with sudo
        let output = Command::new("sudo")
            .args(["-n", "systemctl", systemctl_action, &unit])
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if stderr.is_empty() {
                format!("systemctl {systemctl_action} {unit} failed — may require sudo/root")
            } else {
                stderr
            });
        }
    }
    Ok(())
}

// ── Cross-platform ──────────────────────────────────────────────

fn control_selected(state: &mut HealthState, action: &str) {
    let Some(service) = state.services.get(state.selected) else { return };
    let name = service.name.clone();
    match control_service(&name, action) {
        Ok(()) => {
            state.error = None;
            state.refresh();
            state.status = format!("{name}: {action} succeeded.");
        }
        Err(e) => {
            state.error = Some(format!("{name}: {action} failed — {e}"));
        }
    }
}

pub fn draw(frame: &mut Frame, area: Rect, state: &HealthState) {
    let rect = widgets::centered_fixed(96, area.height.min(32), area);
    let block = widgets::form_block("System Health Dashboard");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let disk_height = (state.disks.len() as u16 + 3).clamp(4, 8);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(disk_height),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(inner);

    let cpu_text = match state.cpu_percent {
        Some(pct) => format!("CPU Load: {pct:.0}%"),
        None => "CPU Load: unavailable".to_string(),
    };
    frame.render_widget(Paragraph::new(Line::styled(cpu_text, theme::title_style())), chunks[0]);

    let ram_text = if state.ram_total_mb > 0 {
        let pct = (state.ram_used_mb as f64 / state.ram_total_mb as f64) * 100.0;
        format!("RAM: {} MB / {} MB used ({pct:.0}%)", state.ram_used_mb, state.ram_total_mb)
    } else {
        "RAM: unavailable".to_string()
    };
    frame.render_widget(Paragraph::new(Line::styled(ram_text, theme::subtitle_style())), chunks[1]);

    let disk_rows: Vec<Row> = state
        .disks
        .iter()
        .map(|d| {
            let used = d.total_gb - d.free_gb;
            let pct = if d.total_gb > 0.0 { used / d.total_gb * 100.0 } else { 0.0 };
            Row::new(vec![
                d.drive.clone(),
                format!("{used:.1} GB"),
                format!("{:.1} GB", d.total_gb),
                format!("{pct:.0}%"),
            ])
        })
        .collect();
    let disk_table = Table::new(
        disk_rows,
        [Constraint::Length(8), Constraint::Length(12), Constraint::Length(12), Constraint::Length(10)],
    )
    .header(Row::new(vec!["Mount", "Used", "Total", "Used %"]).style(theme::title_style()))
    .block(widgets::form_block("Disks"));
    frame.render_widget(disk_table, chunks[2]);

    let service_rows: Vec<Row> = state
        .services
        .iter()
        .map(|s| Row::new(vec![s.name.clone(), s.display_name.clone(), s.status.clone()]))
        .collect();
    let service_table = Table::new(
        service_rows,
        [Constraint::Length(24), Constraint::Min(28), Constraint::Length(12)],
    )
    .header(Row::new(vec!["Name", "Display Name", "Status"]).style(theme::title_style()))
    .row_highlight_style(theme::focused_field_style())
    .highlight_symbol("> ")
    .block(widgets::form_block("Services"));
    let mut table_state = TableState::default()
        .with_selected(if state.services.is_empty() { None } else { Some(state.selected) });
    frame.render_stateful_widget(service_table, chunks[3], &mut table_state);

    let mut lines = vec![Line::styled(
        "Up/Down: select  g: start  x: stop  b: restart  r: refresh  Esc: back",
        theme::hint_style(),
    )];
    if let Some(err) = &state.error {
        lines.push(Line::styled(err.clone(), theme::error_style()));
    }
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[4]);
}

pub fn handle_key(state: &mut HealthState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Char('r') => {
            state.refresh();
            Action::None
        }
        KeyCode::Up => {
            if !state.services.is_empty() {
                state.selected = if state.selected == 0 { state.services.len() - 1 } else { state.selected - 1 };
            }
            Action::None
        }
        KeyCode::Down => {
            if !state.services.is_empty() {
                state.selected = (state.selected + 1) % state.services.len();
            }
            Action::None
        }
        KeyCode::Char('g') => {
            control_selected(state, "start");
            Action::None
        }
        KeyCode::Char('x') => {
            control_selected(state, "stop");
            Action::None
        }
        KeyCode::Char('b') => {
            control_selected(state, "restart");
            Action::None
        }
        _ => Action::None,
    }
}