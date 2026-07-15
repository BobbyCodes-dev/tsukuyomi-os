use std::io::stdout;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::SetTitle;
use ratatui::backend::Backend;
use ratatui::Terminal;

use crate::screens;
use crate::store::users::User;

pub enum Screen {
    Setup(screens::setup::SetupState),
    Login(screens::login::LoginState),
    Desktop,
    Sandbox(screens::sandbox::SandboxState),
    Settings(screens::settings::SettingsState),
    Vault(screens::vault::VaultState),
    Connect(screens::connect::ConnectState),
    Assets(screens::assets::AssetsState),
    Updates(screens::updates::UpdatesState),
    Health(screens::health::HealthState),
    Network(screens::network::NetworkState),
    Firewall(screens::firewall::FirewallState),
    Backups(screens::backups::BackupsState),
    RemoteSupport(screens::remote_support::RemoteSupportState),
    Engagements(screens::engagements::EngagementsState),
    ScanRequest(screens::scan_request::ScanRequestState),
    OsintNotes(screens::osint_notes::OsintNotesState),
    Findings(screens::findings::FindingsState),
    Evidence(screens::evidence::EvidenceState),
    Cve(screens::cve::CveState),
    AiAgent(screens::ai_agent::AiAgentState),
}

impl Screen {
    fn window_title(&self) -> &'static str {
        match self {
            Screen::Setup(_) => "Tsukuyomi OS - Setup",
            Screen::Login(_) => "Tsukuyomi OS - Login",
            Screen::Desktop => "Tsukuyomi OS - App",
            Screen::Sandbox(_) => "Tsukuyomi OS - Sandbox",
            Screen::Settings(_) => "Tsukuyomi OS - Settings",
            Screen::Vault(_) => "Tsukuyomi OS - Vault",
            Screen::Connect(_) => "Tsukuyomi OS - Connect",
            Screen::Assets(_) => "Tsukuyomi OS - Assets",
            Screen::Updates(_) => "Tsukuyomi OS - Updates",
            Screen::Health(_) => "Tsukuyomi OS - Health",
            Screen::Network(_) => "Tsukuyomi OS - Network",
            Screen::Firewall(_) => "Tsukuyomi OS - Firewall",
            Screen::Backups(_) => "Tsukuyomi OS - Backups",
            Screen::RemoteSupport(_) => "Tsukuyomi OS - Remote Support",
            Screen::Engagements(_) => "Tsukuyomi OS - Engagements",
            Screen::ScanRequest(_) => "Tsukuyomi OS - Scan Request",
            Screen::OsintNotes(_) => "Tsukuyomi OS - OSINT Notes",
            Screen::Findings(_) => "Tsukuyomi OS - Findings / Reports",
            Screen::Evidence(_) => "Tsukuyomi OS - Evidence Vault",
            Screen::Cve(_) => "Tsukuyomi OS - CVE Lookup",
            Screen::AiAgent(_) => "Tsukuyomi OS - AI Agent",
        }
    }
}

#[derive(Debug)]
pub enum Action {
    None,
    Quit,
    ToSetup,
    ToLogin,
    LoggedIn(User, String),
    ToSandbox,
    ToSettings,
    ToVault,
    ToConnect,
    ToAssets,
    ToUpdates,
    ToHealth,
    ToNetwork,
    ToFirewall,
    ToBackups,
    ToRemoteSupport,
    ToEngagements,
    ToScanRequest,
    ToOsintNotes,
    ToFindings,
    ToEvidence,
    ToCve,
    ToAiAgent,
    Back,
}

pub struct App {
    pub screen: Screen,
    pub desktop: Option<screens::desktop::DesktopState>,
    pub current_user: Option<User>,
    pub vault_key: Option<[u8; 32]>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        let mut settings = crate::store::settings::load_settings();
        let existing_users = crate::store::users::list_users()?;
        let screen = if !settings.onboarded || existing_users.is_empty() {
            crate::store::users::delete_all_users()?;
            settings.onboarded = false;
            crate::store::settings::save_settings(&settings)?;
            Screen::Setup(screens::setup::SetupState::default())
        } else {
            Screen::Login(screens::login::LoginState::new())
        };
        Ok(App { screen, desktop: None, current_user: None, vault_key: None, should_quit: false })
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => return,
            Action::Quit => self.should_quit = true,
            Action::ToSetup => self.screen = Screen::Setup(screens::setup::SetupState::default()),
            Action::ToLogin => self.screen = Screen::Login(screens::login::LoginState::new()),
            Action::LoggedIn(user, password) => {
                self.vault_key = crate::store::vault::derive_key(user.id, &password).ok();
                self.current_user = Some(user);
                self.desktop = Some(screens::desktop::DesktopState::new());
                self.screen = Screen::Desktop;
            }
            Action::ToSandbox => self.screen = Screen::Sandbox(screens::sandbox::SandboxState::new()),
            Action::ToSettings => self.screen = Screen::Settings(screens::settings::SettingsState::default()),
            Action::ToVault => {
                if let (Some(user), Some(key)) = (self.current_user.clone(), self.vault_key) {
                    self.screen = Screen::Vault(screens::vault::VaultState::new(user.id, key));
                } else if let Some(desktop) = &mut self.desktop {
                    desktop.log_status("Vault unavailable: unable to derive encryption key.");
                }
            }
            Action::ToConnect => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Connect(screens::connect::ConnectState::new(user.id, self.vault_key));
                }
            }
            Action::ToAssets => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Assets(screens::assets::AssetsState::new(user.id));
                }
            }
            Action::ToUpdates => self.screen = Screen::Updates(screens::updates::UpdatesState::new()),
            Action::ToHealth => self.screen = Screen::Health(screens::health::HealthState::new()),
            Action::ToNetwork => self.screen = Screen::Network(screens::network::NetworkState::new()),
            Action::ToFirewall => self.screen = Screen::Firewall(screens::firewall::FirewallState::new()),
            Action::ToBackups => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Backups(screens::backups::BackupsState::new(user.id));
                }
            }
            Action::ToRemoteSupport => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::RemoteSupport(screens::remote_support::RemoteSupportState::new(user.id));
                }
            }
            Action::ToEngagements => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Engagements(screens::engagements::EngagementsState::new(user.id));
                }
            }
            Action::ToScanRequest => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::ScanRequest(screens::scan_request::ScanRequestState::new(user.id));
                }
            }
            Action::ToOsintNotes => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::OsintNotes(screens::osint_notes::OsintNotesState::new(user.id));
                }
            }
            Action::ToFindings => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Findings(screens::findings::FindingsState::new(user.id));
                }
            }
            Action::ToEvidence => {
                if let (Some(user), Some(key)) = (self.current_user.clone(), self.vault_key) {
                    self.screen = Screen::Evidence(screens::evidence::EvidenceState::new(user.id, key));
                } else if let Some(desktop) = &mut self.desktop {
                    desktop.log_status("Evidence Vault unavailable: unable to derive encryption key.");
                }
            }
            Action::ToCve => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Cve(screens::cve::CveState::new(user.id));
                }
            }
            Action::ToAiAgent => {
                if let (Some(user), Some(key)) = (self.current_user.clone(), self.vault_key) {
                    self.screen = Screen::AiAgent(screens::ai_agent::AiAgentState::new(user.id, key));
                } else if let Some(desktop) = &mut self.desktop {
                    desktop.log_status("AI Agent unavailable: unable to derive encryption key.");
                }
            }
            Action::Back => self.screen = Screen::Desktop,
        }
        self.set_window_title();
    }

    fn set_window_title(&self) {
        let _ = execute!(stdout(), SetTitle(self.screen.window_title()));
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.should_quit = true;
            return;
        }
        let action = match &mut self.screen {
            Screen::Setup(state) => screens::setup::handle_key(state, key),
            Screen::Login(state) => screens::login::handle_key(state, key),
            Screen::Desktop => match &mut self.desktop {
                Some(state) => screens::desktop::handle_key(state, key),
                None => Action::None,
            },
            Screen::Sandbox(state) => screens::sandbox::handle_key(state, key),
            Screen::Settings(state) => screens::settings::handle_key(state, key),
            Screen::Vault(state) => screens::vault::handle_key(state, key),
            Screen::Connect(state) => screens::connect::handle_key(state, key),
            Screen::Assets(state) => screens::assets::handle_key(state, key),
            Screen::Updates(state) => screens::updates::handle_key(state, key),
            Screen::Health(state) => screens::health::handle_key(state, key),
            Screen::Network(state) => screens::network::handle_key(state, key),
            Screen::Firewall(state) => screens::firewall::handle_key(state, key),
            Screen::Backups(state) => screens::backups::handle_key(state, key),
            Screen::RemoteSupport(state) => screens::remote_support::handle_key(state, key),
            Screen::Engagements(state) => screens::engagements::handle_key(state, key),
            Screen::ScanRequest(state) => screens::scan_request::handle_key(state, key),
            Screen::OsintNotes(state) => screens::osint_notes::handle_key(state, key),
            Screen::Findings(state) => screens::findings::handle_key(state, key),
            Screen::Evidence(state) => screens::evidence::handle_key(state, key),
            Screen::Cve(state) => screens::cve::handle_key(state, key),
            Screen::AiAgent(state) => screens::ai_agent::handle_key(state, key),
        };
        self.apply(action);
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        match &self.screen {
            Screen::Setup(state) => screens::setup::draw(frame, area, state),
            Screen::Login(state) => screens::login::draw(frame, area, state),
            Screen::Desktop => {
                if let (Some(state), Some(user)) = (&self.desktop, &self.current_user) {
                    screens::desktop::draw(frame, area, state, user);
                }
            }
            Screen::Sandbox(state) => screens::sandbox::draw(frame, area, state),
            Screen::Settings(state) => screens::settings::draw(frame, area, state),
            Screen::Vault(state) => screens::vault::draw(frame, area, state),
            Screen::Connect(state) => screens::connect::draw(frame, area, state),
            Screen::Assets(state) => screens::assets::draw(frame, area, state),
            Screen::Updates(state) => screens::updates::draw(frame, area, state),
            Screen::Health(state) => screens::health::draw(frame, area, state),
            Screen::Network(state) => screens::network::draw(frame, area, state),
            Screen::Firewall(state) => screens::firewall::draw(frame, area, state),
            Screen::Backups(state) => screens::backups::draw(frame, area, state),
            Screen::RemoteSupport(state) => screens::remote_support::draw(frame, area, state),
            Screen::Engagements(state) => screens::engagements::draw(frame, area, state),
            Screen::ScanRequest(state) => screens::scan_request::draw(frame, area, state),
            Screen::OsintNotes(state) => screens::osint_notes::draw(frame, area, state),
            Screen::Findings(state) => screens::findings::draw(frame, area, state),
            Screen::Evidence(state) => screens::evidence::draw(frame, area, state),
            Screen::Cve(state) => screens::cve::draw(frame, area, state),
            Screen::AiAgent(state) => screens::ai_agent::draw(frame, area, state),
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        self.set_window_title();
        while !self.should_quit {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key);
                    }
                }
            }

            if let Some(desktop) = &mut self.desktop {
                desktop.tick();
            }

            if let Screen::Sandbox(state) = &mut self.screen {
                state.poll_build();
            }

            if let Screen::Assets(state) = &mut self.screen {
                state.poll_ping();
            }

            if let Screen::Network(state) = &mut self.screen {
                state.poll_diag();
            }

            if let Screen::Backups(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::RemoteSupport(state) = &mut self.screen {
                state.poll_download();
            }

            if let Screen::ScanRequest(state) = &mut self.screen {
                state.poll_submit();
            }

            if let Screen::Cve(state) = &mut self.screen {
                state.poll_fetch();
            }

            if let Screen::AiAgent(state) = &mut self.screen {
                state.poll();
            }
        }
        Ok(())
    }
}
