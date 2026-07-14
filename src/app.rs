use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
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
}

pub enum Action {
    None,
    Quit,
    ToSetup,
    ToLogin,
    LoggedIn(User),
    ToSandbox,
    ToSettings,
    Back,
}

pub struct App {
    pub screen: Screen,
    pub desktop: Option<screens::desktop::DesktopState>,
    pub current_user: Option<User>,
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
        Ok(App { screen, desktop: None, current_user: None, should_quit: false })
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.should_quit = true,
            Action::ToSetup => self.screen = Screen::Setup(screens::setup::SetupState::default()),
            Action::ToLogin => self.screen = Screen::Login(screens::login::LoginState::new()),
            Action::LoggedIn(user) => {
                self.current_user = Some(user);
                self.desktop = Some(screens::desktop::DesktopState::new());
                self.screen = Screen::Desktop;
            }
            Action::ToSandbox => self.screen = Screen::Sandbox(screens::sandbox::SandboxState::new()),
            Action::ToSettings => self.screen = Screen::Settings(screens::settings::SettingsState::default()),
            Action::Back => self.screen = Screen::Desktop,
        }
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
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
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
        }
        Ok(())
    }
}
