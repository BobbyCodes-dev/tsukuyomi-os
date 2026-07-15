use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::ai::{self, AiProvider, ProviderKind};
use crate::store::settings;
use crate::store::vault::{self, VaultKey};
use crate::ui::{theme, widgets};
use crate::vm::network::ALL_MODES;

const FIELD_COUNT: usize = 11;
const AI_KIND_FIELD: usize = 6;
const AI_MODEL_FIELD: usize = 7;
const AI_ENDPOINT_FIELD: usize = 8;
const AI_API_KEY_FIELD: usize = 9;
const NUKE_FIELD: usize = 10;

const PROVIDER_KINDS: [ProviderKind; 5] = [
    ProviderKind::Anthropic,
    ProviderKind::OpenAiCompatible,
    ProviderKind::Gemini,
    ProviderKind::Ollama,
    ProviderKind::OllamaCloud,
];

fn provider_kind_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Anthropic => "Anthropic",
        ProviderKind::OpenAiCompatible => "OpenAI-Compatible",
        ProviderKind::Gemini => "Gemini",
        ProviderKind::Ollama => "Ollama (local)",
        ProviderKind::OllamaCloud => "Ollama Cloud",
    }
}

fn default_model_endpoint(kind: ProviderKind) -> (&'static str, &'static str) {
    match kind {
        ProviderKind::Anthropic => ("claude-3-5-sonnet-20241022", "https://api.anthropic.com/v1/messages"),
        ProviderKind::OpenAiCompatible => ("gpt-4o-mini", "https://api.openai.com/v1/chat/completions"),
        ProviderKind::Gemini => ("gemini-1.5-flash", "https://generativelanguage.googleapis.com/v1beta/models"),
        ProviderKind::Ollama => ("llama3.2", "http://localhost:11434/api/chat"),
        ProviderKind::OllamaCloud => ("gpt-oss:120b", "https://ollama.com/api/chat"),
    }
}

fn vault_entry_name(kind: ProviderKind) -> String {
    format!("ai-provider-{:?}", kind).to_lowercase()
}

pub struct SettingsState {
    pub theme: widgets::TextField,
    pub timezone: widgets::TextField,
    pub language: widgets::TextField,
    pub notifications: widgets::TextField,
    pub show_security_tools: bool,
    pub network_mode_idx: usize,
    pub ai_provider_id: i64,
    pub ai_kind_idx: usize,
    pub ai_model: widgets::TextField,
    pub ai_available_models: Vec<String>,
    pub ai_endpoint: widgets::TextField,
    pub ai_api_key: widgets::TextField,
    pub user_id: i64,
    pub vault_key: Option<VaultKey>,
    pub nuke_input: widgets::TextField,
    pub focus: usize,
    pub status: String,
}

impl SettingsState {
    pub fn new(user_id: i64, vault_key: Option<VaultKey>) -> Self {
        let s = settings::load_settings();
        let network_mode_idx = ALL_MODES
            .iter()
            .position(|m| m.id() == s.vm_network_mode)
            .unwrap_or(0);

        let provider = ai::load_provider(user_id).ok().flatten();
        let ai_kind_idx = provider
            .as_ref()
            .and_then(|p| PROVIDER_KINDS.iter().position(|k| *k == p.kind))
            .unwrap_or(0);
        let defaults = default_model_endpoint(PROVIDER_KINDS[ai_kind_idx]);
        let ai_model = provider.as_ref().map(|p| p.model.clone()).unwrap_or_else(|| defaults.0.to_string());
        let ai_endpoint = provider.as_ref().map(|p| p.endpoint.clone()).unwrap_or_else(|| defaults.1.to_string());
        let ai_provider_id = provider.as_ref().map(|p| p.id).unwrap_or(0);

        let existing_key = vault_key.and_then(|key| {
            let kind = provider.as_ref().map(|p| p.kind).unwrap_or(ProviderKind::Anthropic);
            vault::get_entry_by_name(user_id, &key, &vault_entry_name(kind)).ok().flatten().map(|e| e.password)
        });
        let existing_key = existing_key.unwrap_or_default();

        let (ai_available_models, _, _) = fetch_models_or_fallback(
            PROVIDER_KINDS[ai_kind_idx],
            &ai_endpoint,
            &existing_key,
            &ai_model,
        );

        SettingsState {
            theme: widgets::TextField::with_value(s.theme),
            timezone: widgets::TextField::with_value(s.timezone),
            language: widgets::TextField::with_value(s.language),
            notifications: widgets::TextField::with_value(if s.notifications { "true" } else { "false" }),
            show_security_tools: s.show_security_tools,
            network_mode_idx,
            ai_provider_id,
            ai_kind_idx,
            ai_model: widgets::TextField::with_value(ai_model),
            ai_available_models,
            ai_endpoint: widgets::TextField::with_value(ai_endpoint),
            ai_api_key: widgets::TextField { value: existing_key, masked: true },
            user_id,
            vault_key,
            nuke_input: widgets::TextField::new(),
            focus: 0,
            status: String::new(),
        }
    }
}

fn fetch_models_or_fallback(kind: ProviderKind, endpoint: &str, api_key: &str, fallback_model: &str) -> (Vec<String>, bool, String) {
    let mut note = String::new();
    if kind == ProviderKind::Ollama && !crate::ollama_setup::is_reachable() {
        note = match crate::ollama_setup::ensure_running() {
            crate::ollama_setup::EnsureResult::AlreadyRunning => String::new(),
            crate::ollama_setup::EnsureResult::Started => " Started local Ollama automatically.".to_string(),
            crate::ollama_setup::EnsureResult::StartFailed => {
                " Ollama is installed but failed to start; try starting it manually.".to_string()
            }
            crate::ollama_setup::EnsureResult::NotInstalled => {
                " Ollama isn't installed. Press 'i' on the Provider field to download and install it.".to_string()
            }
        };
    }
    let should_attempt = kind == ProviderKind::Ollama || !api_key.is_empty();
    if should_attempt {
        if let Ok(models) = crate::store::ai_client::fetch_models_blocking(kind, endpoint, api_key) {
            if !models.is_empty() {
                return (models, true, note);
            }
        }
    }
    (vec![fallback_model.to_string()], false, note)
}

fn model_field_label(state: &SettingsState) -> String {
    if state.ai_available_models.len() > 1 {
        let idx = state.ai_available_models.iter().position(|m| m == &state.ai_model.value).unwrap_or(0);
        format!("Model [{}/{}]", idx + 1, state.ai_available_models.len())
    } else {
        "Model".to_string()
    }
}

fn field_line(label: &str, value: String, focused: bool) -> Line<'static> {
    let prefix = if focused { "> " } else { "  " };
    let style = if focused { theme::focused_field_style() } else { Style::default() };
    Line::from(vec![Span::styled(format!("{prefix}{label}: "), style), Span::raw(value)])
}

pub fn draw(frame: &mut Frame, area: Rect, state: &SettingsState) {
    let rect = widgets::centered_fixed(80, 28, area);
    let block = widgets::form_block("Settings");
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let nuke_focused = state.focus == NUKE_FIELD;
    let nuke_prefix = if nuke_focused { "> " } else { "  " };
    let nuke_style = if nuke_focused { theme::error_style() } else { Style::default() };

    let mut lines = vec![
        field_line("Theme", state.theme.display(), state.focus == 0),
        field_line("Timezone", state.timezone.display(), state.focus == 1),
        field_line("Language", state.language.display(), state.focus == 2),
        field_line("Notifications", state.notifications.display(), state.focus == 3),
        field_line(
            "Show Security Tools",
            if state.show_security_tools { "yes".to_string() } else { "no".to_string() },
            state.focus == 4,
        ),
        field_line(
            "Sandbox VM Network",
            ALL_MODES[state.network_mode_idx].label().to_string(),
            state.focus == 5,
        ),
        Line::raw(""),
        Line::styled("AI Agent Provider", theme::title_style()),
        field_line(
            "Provider",
            provider_kind_label(PROVIDER_KINDS[state.ai_kind_idx]).to_string(),
            state.focus == AI_KIND_FIELD,
        ),
    ];
    if PROVIDER_KINDS[state.ai_kind_idx] == ProviderKind::Ollama && crate::ollama_setup::installed_path().is_none() {
        lines.push(Line::styled(
            "  Ollama not found on this PC. Press 'i' here to download and install it.",
            theme::error_style(),
        ));
    }
    lines.extend(vec![
        field_line(
            &model_field_label(state),
            state.ai_model.display(),
            state.focus == AI_MODEL_FIELD,
        ),
        field_line("Endpoint", state.ai_endpoint.display(), state.focus == AI_ENDPOINT_FIELD),
        field_line("API Key", state.ai_api_key.display(), state.focus == AI_API_KEY_FIELD),
        Line::raw(""),
        Line::styled("Danger Zone", theme::error_style()),
        Line::styled(
            "Erases all Tsukuyomi OS data (accounts, vault, VMs, settings). The exe itself is kept.",
            theme::hint_style(),
        ),
        Line::from(vec![
            Span::styled(format!("{nuke_prefix}Type NUKE to erase all data: "), nuke_style),
            Span::raw(state.nuke_input.display()),
        ]),
        Line::raw(""),
        Line::styled("Tab: move  Left/Right: change  Enter: save/confirm  Esc: back", theme::hint_style()),
    ]);
    if !state.status.is_empty() {
        lines.push(Line::styled(state.status.clone(), theme::clock_style()));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn on_ai_kind_changed(state: &mut SettingsState) {
    let kind = PROVIDER_KINDS[state.ai_kind_idx];
    let (default_model, default_endpoint) = default_model_endpoint(kind);
    state.ai_endpoint.value = default_endpoint.to_string();
    refresh_ai_models(state, default_model);
}

fn refresh_ai_models(state: &mut SettingsState, fallback_model: &str) {
    let kind = PROVIDER_KINDS[state.ai_kind_idx];
    let (models, fetched, note) =
        fetch_models_or_fallback(kind, &state.ai_endpoint.value, &state.ai_api_key.value, fallback_model);
    state.ai_model.value = models[0].clone();
    state.status = if fetched {
        format!("Fetched {} model(s) for {}.{note}", models.len(), provider_kind_label(kind))
    } else {
        format!("Could not list models for {}; using default.{note}", provider_kind_label(kind))
    };
    state.ai_available_models = models;
}

fn save(state: &mut SettingsState) {
    let mut s = settings::load_settings();
    s.theme = state.theme.value.clone();
    s.timezone = state.timezone.value.clone();
    s.language = state.language.value.clone();
    s.notifications = matches!(state.notifications.value.to_lowercase().as_str(), "true" | "1" | "yes" | "on");
    s.show_security_tools = state.show_security_tools;
    s.vm_network_mode = ALL_MODES[state.network_mode_idx].id().to_string();

    let mut messages = Vec::new();
    match settings::save_settings(&s) {
        Ok(()) => messages.push("Settings saved.".to_string()),
        Err(e) => messages.push(format!("Error saving settings: {e}")),
    }

    let kind = PROVIDER_KINDS[state.ai_kind_idx];
    let entry_name = vault_entry_name(kind);
    let provider = AiProvider {
        id: state.ai_provider_id,
        kind,
        model: state.ai_model.value.trim().to_string(),
        endpoint: state.ai_endpoint.value.trim().to_string(),
        vault_label: entry_name.clone(),
        is_default: true,
    };
    match ai::save_provider(state.user_id, &provider) {
        Ok(id) => {
            state.ai_provider_id = id;
            if state.ai_api_key.value.is_empty() {
                messages.push("AI provider saved.".to_string());
            } else {
                match state.vault_key {
                    Some(key) => match vault::upsert_entry_by_name(
                        state.user_id,
                        &key,
                        &entry_name,
                        "api-key",
                        &state.ai_api_key.value,
                        "AI provider API key",
                    ) {
                        Ok(()) => messages.push("AI provider saved.".to_string()),
                        Err(e) => messages.push(format!("Failed to save API key: {e}")),
                    },
                    None => messages.push(
                        "AI provider metadata saved, but the vault key is unavailable — log out and back in to save the API key.".to_string(),
                    ),
                }
            }
        }
        Err(e) => messages.push(format!("Failed to save AI provider: {e}")),
    }

    state.status = messages.join(" ");
}

pub fn handle_key(state: &mut SettingsState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc => Action::Back,
        KeyCode::Tab | KeyCode::Down => {
            let leaving_key_field = state.focus == AI_API_KEY_FIELD;
            state.focus = (state.focus + 1) % FIELD_COUNT;
            if leaving_key_field && !state.ai_api_key.value.is_empty() {
                let fallback = state.ai_model.value.clone();
                refresh_ai_models(state, &fallback);
            }
            Action::None
        }
        KeyCode::BackTab | KeyCode::Up => {
            let leaving_key_field = state.focus == AI_API_KEY_FIELD;
            state.focus = (state.focus + FIELD_COUNT - 1) % FIELD_COUNT;
            if leaving_key_field && !state.ai_api_key.value.is_empty() {
                let fallback = state.ai_model.value.clone();
                refresh_ai_models(state, &fallback);
            }
            Action::None
        }
        KeyCode::Left if state.focus == 4 => {
            state.show_security_tools = !state.show_security_tools;
            Action::None
        }
        KeyCode::Right if state.focus == 4 => {
            state.show_security_tools = !state.show_security_tools;
            Action::None
        }
        KeyCode::Left if state.focus == 5 => {
            state.network_mode_idx = (state.network_mode_idx + ALL_MODES.len() - 1) % ALL_MODES.len();
            Action::None
        }
        KeyCode::Right if state.focus == 5 => {
            state.network_mode_idx = (state.network_mode_idx + 1) % ALL_MODES.len();
            Action::None
        }
        KeyCode::Left if state.focus == AI_KIND_FIELD => {
            state.ai_kind_idx = (state.ai_kind_idx + PROVIDER_KINDS.len() - 1) % PROVIDER_KINDS.len();
            on_ai_kind_changed(state);
            Action::None
        }
        KeyCode::Right if state.focus == AI_KIND_FIELD => {
            state.ai_kind_idx = (state.ai_kind_idx + 1) % PROVIDER_KINDS.len();
            on_ai_kind_changed(state);
            Action::None
        }
        KeyCode::Char('i')
            if state.focus == AI_KIND_FIELD && PROVIDER_KINDS[state.ai_kind_idx] == ProviderKind::Ollama =>
        {
            state.status = "Downloading Ollama installer...".to_string();
            match crate::ollama_setup::download_and_launch_installer() {
                Ok(_) => {
                    state.status =
                        "Ollama installer launched. Finish the install, then press Left/Right on Provider to retry."
                            .to_string()
                }
                Err(e) => state.status = format!("Failed to download Ollama installer: {e}"),
            }
            Action::None
        }
        KeyCode::Left if state.focus == AI_MODEL_FIELD && state.ai_available_models.len() > 1 => {
            let idx = state.ai_available_models.iter().position(|m| m == &state.ai_model.value).unwrap_or(0);
            let n = state.ai_available_models.len();
            state.ai_model.value = state.ai_available_models[(idx + n - 1) % n].clone();
            Action::None
        }
        KeyCode::Right if state.focus == AI_MODEL_FIELD && state.ai_available_models.len() > 1 => {
            let idx = state.ai_available_models.iter().position(|m| m == &state.ai_model.value).unwrap_or(0);
            let n = state.ai_available_models.len();
            state.ai_model.value = state.ai_available_models[(idx + 1) % n].clone();
            Action::None
        }
        KeyCode::Enter if state.focus == NUKE_FIELD => {
            if state.nuke_input.value == "NUKE" {
                let messages = crate::uninstall::nuke_data(false);
                state.status = messages.join(" ");
                Action::Quit
            } else {
                state.status = "Type NUKE (all caps) in the field above, then press Enter to confirm.".to_string();
                Action::None
            }
        }
        KeyCode::Enter => {
            save(state);
            Action::None
        }
        KeyCode::Backspace => {
            match state.focus {
                0 => state.theme.backspace(),
                1 => state.timezone.backspace(),
                2 => state.language.backspace(),
                3 => state.notifications.backspace(),
                AI_MODEL_FIELD => state.ai_model.backspace(),
                AI_ENDPOINT_FIELD => state.ai_endpoint.backspace(),
                AI_API_KEY_FIELD => state.ai_api_key.backspace(),
                NUKE_FIELD => state.nuke_input.backspace(),
                _ => {}
            }
            Action::None
        }
        KeyCode::Char(c) => {
            match state.focus {
                0 => state.theme.push_char(c),
                1 => state.timezone.push_char(c),
                2 => state.language.push_char(c),
                3 => state.notifications.push_char(c),
                AI_MODEL_FIELD => state.ai_model.push_char(c),
                AI_ENDPOINT_FIELD => state.ai_endpoint.push_char(c),
                AI_API_KEY_FIELD => state.ai_api_key.push_char(c),
                NUKE_FIELD => state.nuke_input.push_char(c),
                _ => {}
            }
            Action::None
        }
        _ => Action::None,
    }
}
