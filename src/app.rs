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
    VoidAccess(screens::voidaccess::VoidAccessState),
    Aimap(screens::aimap::AimapState),
    Netryx(screens::netryx::NetryxState),
    PhoneInfoga(screens::phoneinfoga::PhoneInfogaState),
    Fawkes(screens::fawkes::FawkesState),
    ParamSpider(screens::paramspider::ParamSpiderState),
    Photon(screens::photon::PhotonState),
    OnionShare(screens::onionshare::OnionShareState),
    ReconFtw(screens::reconftw::ReconFtwState),
    Canarytokens(screens::canarytokens::CanarytokensState),
    John(screens::john::JohnState),
    Hashcat(screens::hashcat::HashcatState),
    Hydra(screens::hydra::HydraState),
    Hashid(screens::hashid::HashidState),
    Crunch(screens::crunch::CrunchState),
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
            Screen::VoidAccess(_) => "Tsukuyomi OS - VoidAccess (Dark Web OSINT)",
            Screen::Aimap(_) => "Tsukuyomi OS - AIMap (AI Infra Discovery)",
            Screen::Netryx(_) => "Tsukuyomi OS - Netryx (Image Geolocation)",
            Screen::PhoneInfoga(_) => "Tsukuyomi OS - PhoneInfoga (Phone OSINT)",
            Screen::Fawkes(_) => "Tsukuyomi OS - Fawkes (Image Anonymization)",
            Screen::ParamSpider(_) => "Tsukuyomi OS - ParamSpider (URL Parameter Discovery)",
            Screen::Photon(_) => "Tsukuyomi OS - Photon (Web Crawler)",
            Screen::OnionShare(_) => "Tsukuyomi OS - OnionShare (Anonymous File Sharing)",
            Screen::ReconFtw(_) => "Tsukuyomi OS - reconFTW (Reconnaissance Framework)",
            Screen::Canarytokens(_) => "Tsukuyomi OS - Canarytokens (Token Generation)",
            Screen::John(_) => "Tsukuyomi OS - John the Ripper (Password Cracking)",
            Screen::Hashcat(_) => "Tsukuyomi OS - Hashcat (GPU Password Recovery)",
            Screen::Hydra(_) => "Tsukuyomi OS - Hydra (Network Brute Force)",
            Screen::Hashid(_) => "Tsukuyomi OS - Hashid (Hash Identification)",
            Screen::Crunch(_) => "Tsukuyomi OS - Crunch (Wordlist Generator)",
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
    ToVoidAccess,
    ToAimap,
    ToNetryx,
    ToPhoneInfoga,
    ToFawkes,
    ToParamSpider,
    ToPhoton,
    ToOnionShare,
    ToReconFtw,
    ToCanarytokens,
    ToJohn,
    ToHashcat,
    ToHydra,
    ToHashid,
    ToCrunch,
    Back,
}

pub struct App {
    pub screen: Screen,
    pub desktop: Option<screens::desktop::DesktopState>,
    pub current_user: Option<User>,
    pub vault_key: Option<[u8; 32]>,
    pub should_quit: bool,
    start_ai_agent: bool,
}

impl App {
    pub fn new(start_ai_agent: bool) -> Result<Self> {
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
        Ok(App {
            screen,
            desktop: None,
            current_user: None,
            vault_key: None,
            should_quit: false,
            start_ai_agent,
        })
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => return,
            Action::Quit => self.should_quit = true,
            Action::ToSetup => self.screen = Screen::Setup(screens::setup::SetupState::default()),
            Action::ToLogin => self.screen = Screen::Login(screens::login::LoginState::new()),
            Action::LoggedIn(user, password) => {
                self.vault_key = crate::store::vault::derive_key(user.id, &password).ok();
                self.desktop = Some(screens::desktop::DesktopState::new(user.id, self.vault_key));
                self.screen = if self.start_ai_agent {
                    match self.vault_key {
                        Some(key) => Screen::AiAgent(screens::ai_agent::AiAgentState::new(user.id, key)),
                        None => Screen::Desktop,
                    }
                } else {
                    Screen::Desktop
                };
                self.start_ai_agent = false;
                self.current_user = Some(user);
            }
            Action::ToSandbox => self.screen = Screen::Sandbox(screens::sandbox::SandboxState::new()),
            Action::ToSettings => {
                let user_id = self.current_user.as_ref().map(|u| u.id).unwrap_or(0);
                self.screen = Screen::Settings(screens::settings::SettingsState::new(user_id, self.vault_key));
            }
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
            Action::ToVoidAccess => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::VoidAccess(screens::voidaccess::VoidAccessState::new(user.id));
                }
            }
            Action::ToAimap => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Aimap(screens::aimap::AimapState::new(user.id));
                }
            }
            Action::ToNetryx => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Netryx(screens::netryx::NetryxState::new(user.id));
                }
            }
            Action::ToPhoneInfoga => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::PhoneInfoga(screens::phoneinfoga::PhoneInfogaState::new(user.id));
                }
            }
            Action::ToFawkes => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Fawkes(screens::fawkes::FawkesState::new(user.id));
                }
            }
            Action::ToParamSpider => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::ParamSpider(screens::paramspider::ParamSpiderState::new(user.id));
                }
            }
            Action::ToPhoton => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Photon(screens::photon::PhotonState::new(user.id));
                }
            }
            Action::ToOnionShare => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::OnionShare(screens::onionshare::OnionShareState::new(user.id));
                }
            }
            Action::ToReconFtw => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::ReconFtw(screens::reconftw::ReconFtwState::new(user.id));
                }
            }
            Action::ToCanarytokens => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Canarytokens(screens::canarytokens::CanarytokensState::new(user.id));
                }
            }
            Action::ToJohn => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::John(screens::john::JohnState::new(user.id));
                }
            }
            Action::ToHashcat => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Hashcat(screens::hashcat::HashcatState::new(user.id));
                }
            }
            Action::ToHydra => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Hydra(screens::hydra::HydraState::new(user.id));
                }
            }
            Action::ToHashid => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Hashid(screens::hashid::HashidState::new(user.id));
                }
            }
            Action::ToCrunch => {
                if let Some(user) = self.current_user.clone() {
                    self.screen = Screen::Crunch(screens::crunch::CrunchState::new(user.id));
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
            Screen::VoidAccess(state) => screens::voidaccess::handle_key(state, key),
            Screen::Aimap(state) => screens::aimap::handle_key(state, key),
            Screen::Netryx(state) => screens::netryx::handle_key(state, key),
            Screen::PhoneInfoga(state) => screens::phoneinfoga::handle_key(state, key),
            Screen::Fawkes(state) => screens::fawkes::handle_key(state, key),
            Screen::ParamSpider(state) => screens::paramspider::handle_key(state, key),
            Screen::Photon(state) => screens::photon::handle_key(state, key),
            Screen::OnionShare(state) => screens::onionshare::handle_key(state, key),
            Screen::ReconFtw(state) => screens::reconftw::handle_key(state, key),
            Screen::Canarytokens(state) => screens::canarytokens::handle_key(state, key),
            Screen::John(state) => screens::john::handle_key(state, key),
            Screen::Hashcat(state) => screens::hashcat::handle_key(state, key),
            Screen::Hydra(state) => screens::hydra::handle_key(state, key),
            Screen::Hashid(state) => screens::hashid::handle_key(state, key),
            Screen::Crunch(state) => screens::crunch::handle_key(state, key),
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
            Screen::VoidAccess(state) => screens::voidaccess::draw(frame, area, state),
            Screen::Aimap(state) => screens::aimap::draw(frame, area, state),
            Screen::Netryx(state) => screens::netryx::draw(frame, area, state),
            Screen::PhoneInfoga(state) => screens::phoneinfoga::draw(frame, area, state),
            Screen::Fawkes(state) => screens::fawkes::draw(frame, area, state),
            Screen::ParamSpider(state) => screens::paramspider::draw(frame, area, state),
            Screen::Photon(state) => screens::photon::draw(frame, area, state),
            Screen::OnionShare(state) => screens::onionshare::draw(frame, area, state),
            Screen::ReconFtw(state) => screens::reconftw::draw(frame, area, state),
            Screen::Canarytokens(state) => screens::canarytokens::draw(frame, area, state),
            Screen::John(state) => screens::john::draw(frame, area, state),
            Screen::Hashcat(state) => screens::hashcat::draw(frame, area, state),
            Screen::Hydra(state) => screens::hydra::draw(frame, area, state),
            Screen::Hashid(state) => screens::hashid::draw(frame, area, state),
            Screen::Crunch(state) => screens::crunch::draw(frame, area, state),
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
                desktop.poll_ai();
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

            if let Screen::VoidAccess(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Aimap(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Netryx(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::PhoneInfoga(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Fawkes(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::ParamSpider(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Photon(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::OnionShare(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::ReconFtw(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Canarytokens(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::John(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Hashcat(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Hydra(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Hashid(state) = &mut self.screen {
                state.poll_run();
            }

            if let Screen::Crunch(state) = &mut self.screen {
                state.poll_run();
            }
        }
        Ok(())
    }
}
