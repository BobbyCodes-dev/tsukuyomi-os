use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::ai::{AiProvider, ProviderKind, load_provider, save_provider};
use crate::store::ai_client::{ChatMessage, AiResponse};
use crate::store::ai_tools::{all_tools, dispatch, DispatcherState};
use crate::store::vault::{VaultKey, add_entry, get_entry_by_name};
use crate::ui::{theme, widgets};

#[derive(Debug, Clone, PartialEq)]
enum Mode {
    Chat,
    Settings,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub text: String,
}

impl Message {
    fn is_tool(&self) -> bool {
        self.role == "tool"
    }
}

#[derive(Debug)]
pub struct AiAgentState {
    user_id: i64,
    vault_key: VaultKey,
    mode: Mode,
    messages: Vec<Message>,
    input: widgets::TextField,
    scroll: u16,
    busy: bool,
    error: String,
    provider: Option<AiProvider>,
    settings_fields: Vec<widgets::TextField>,
    settings_focus: usize,
    settings_saved: bool,
    dispatcher: DispatcherState,
}

impl AiAgentState {
    pub fn new(user_id: i64, vault_key: VaultKey) -> Self {
        let provider = load_provider(user_id).ok().flatten();
        let mut state = Self {
            user_id,
            vault_key,
            mode: Mode::Chat,
            messages: vec![Message {
                role: "system".to_string(),
                text: "You are Tsukuyomi AI, an assistant inside a local security-focused OS. You can open apps and answer questions about the OS.".to_string(),
            }],
            input: widgets::TextField::new(),
            scroll: 0,
            busy: false,
            error: String::new(),
            provider: provider.clone(),
            settings_fields: vec![
                widgets::TextField::with_value(provider.as_ref().map(|p| format!("{:?}", p.kind)).unwrap_or_else(|| "Anthropic".to_string())),
                widgets::TextField::with_value(provider.as_ref().map(|p| p.model.clone()).unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_string())),
                widgets::TextField::with_value(provider.as_ref().map(|p| p.endpoint.clone()).unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string())),
                widgets::TextField::masked_field(),
            ],
            settings_focus: 0,
            settings_saved: false,
            dispatcher: DispatcherState::new(),
        };
        if state.provider.is_none() {
            state.error = "No AI provider configured. Press 's' to set provider, model, endpoint and API key.".to_string();
        }
        state
    }

    fn current_provider(&self) -> Option<(&AiProvider, String)> {
        let provider = self.provider.as_ref()?;
        let entry_name = vault_entry_name(provider);
        let api_key = get_entry_by_name(self.user_id, &self.vault_key, &entry_name)
            .ok()
            .flatten()
            .map(|e| e.password)
            .unwrap_or_default();
        if api_key.is_empty() {
            return None;
        }
        Some((provider, api_key))
    }

    pub fn poll(&mut self) {
        // Async work would be here; kept synchronous in this pass.
    }
}

fn vault_entry_name(provider: &AiProvider) -> String {
    format!("ai-provider-{:?}", provider.kind).to_lowercase()
}

fn kind_from_str(s: &str) -> ProviderKind {
    match s.to_lowercase().as_str() {
        "openai" | "openai-compatible" | "openai compatible" => ProviderKind::OpenAiCompatible,
        "gemini" => ProviderKind::Gemini,
        "ollama" => ProviderKind::Ollama,
        _ => ProviderKind::Anthropic,
    }
}

pub fn handle_key(state: &mut AiAgentState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Esc {
        return Action::Back;
    }
    match state.mode {
        Mode::Chat => handle_chat_key(state, key),
        Mode::Settings => handle_settings_key(state, key),
    }
}

fn handle_chat_key(state: &mut AiAgentState, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Char('s') => {
            state.mode = Mode::Settings;
            state.error.clear();
            return Action::None;
        }
        KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
            return Action::Quit;
        }
        KeyCode::Enter => {
            let text = state.input.value.trim().to_string();
            if !text.is_empty() && !state.busy {
                state.input.value.clear();
                state.messages.push(Message { role: "user".to_string(), text });
                state.scroll = u16::MAX;
                submit_chat(state);
            }
        }
        KeyCode::Char(c) => state.input.push_char(c),
        KeyCode::Backspace => state.input.backspace(),
        KeyCode::Up => {
            state.scroll = state.scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            state.scroll = state.scroll.saturating_add(1);
        }
        _ => {}
    }
    Action::None
}

fn handle_settings_key(state: &mut AiAgentState, key: KeyEvent) -> Action {
    const FIELD_COUNT: usize = 4;
    match key.code {
        KeyCode::Esc | KeyCode::Char('s') => {
            state.mode = Mode::Chat;
            return Action::None;
        }
        KeyCode::Tab => {
            state.settings_focus = (state.settings_focus + 1) % FIELD_COUNT;
        }
        KeyCode::BackTab => {
            state.settings_focus = (state.settings_focus + FIELD_COUNT - 1) % FIELD_COUNT;
        }
        KeyCode::Enter => {
            save_ai_settings(state);
        }
        KeyCode::Char(c) => state.settings_fields[state.settings_focus].push_char(c),
        KeyCode::Backspace => state.settings_fields[state.settings_focus].backspace(),
        _ => {}
    }
    Action::None
}

fn save_ai_settings(state: &mut AiAgentState) {
    let kind = kind_from_str(&state.settings_fields[0].value);
    let model = state.settings_fields[1].value.trim().to_string();
    let endpoint = state.settings_fields[2].value.trim().to_string();
    let api_key = state.settings_fields[3].value.clone();
    let tmp_provider = AiProvider {
        kind,
        model: model.clone(),
        endpoint: endpoint.clone(),
        id: 0,
        is_default: true,
        vault_label: String::new(),
    };
    let entry_name = vault_entry_name(&tmp_provider);
    let provider = AiProvider {
        kind: tmp_provider.kind,
        model,
        endpoint,
        id: 0,
        is_default: true,
        vault_label: entry_name.clone(),
    };
    state.error.clear();
    let mut ok = true;
    if let Err(e) = save_provider(state.user_id, &provider) {
        state.error = format!("Failed to save provider: {}", e);
        ok = false;
    }
    if !api_key.is_empty() {
        if let Err(e) = add_entry(
            state.user_id,
            &state.vault_key,
            &entry_name,
            "api-key",
            &api_key,
            "AI provider API key",
        ) {
            state.error = format!("Failed to save API key: {}", e);
            ok = false;
        }
    }
    if ok {
        state.provider = Some(provider);
        state.settings_saved = true;
    }
}

fn submit_chat(state: &mut AiAgentState) {
    let (provider, api_key) = match state.current_provider() {
        Some(p) => p,
        None => {
            state.error = "No AI provider configured. Press 's' to configure.".to_string();
            state.messages.push(Message {
                role: "assistant".to_string(),
                text: "AI provider is not configured. Press 's' to set provider, model, endpoint, and API key.".to_string(),
            });
            return;
        }
    };
    let client = crate::store::ai_client::build_client(provider, &api_key);
    let tools = all_tools();
    let chat_messages: Vec<ChatMessage> = state
        .messages
        .iter()
        .filter(|m| !m.is_tool())
        .map(|m| ChatMessage { role: m.role.clone(), content: m.text.clone() })
        .collect();
    match std::thread::scope(|s| {
        s.spawn(|| {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(client.complete(&chat_messages, &tools))
        })
        .join()
    }) {
        Ok(Ok(resp)) => handle_response(state, resp),
        Ok(Err(e)) => state.error = format!("AI request failed: {}", e),
        Err(e) => state.error = format!("AI thread panicked: {:?}", e),
    }
}

fn handle_response(state: &mut AiAgentState, resp: AiResponse) {
    if !resp.content.is_empty() {
        state.messages.push(Message { role: "assistant".to_string(), text: resp.content });
    }
    if !resp.tool_calls.is_empty() {
        let mut tool_results = Vec::new();
        for call in &resp.tool_calls {
            match dispatch(&mut state.dispatcher, call) {
                Ok(result) => {
                    tool_results.push(result.clone());
                    state.messages.push(Message {
                        role: "tool".to_string(),
                        text: format!("{}: {}", result.name, result.output),
                    });
                }
                Err(e) => {
                    state.messages.push(Message {
                        role: "tool".to_string(),
                        text: format!("Error dispatching tool {}: {}", call.name, e),
                    });
                }
            }
        }
        if state.dispatcher.requested_action.is_some() {
            // Action request surfaced to app; the assistant message already noted it.
        }
    }
    state.scroll = u16::MAX;
}

pub fn draw(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3), Constraint::Length(1)])
        .split(area);
    draw_messages(frame, chunks[0], state);
    draw_input(frame, chunks[1], state);
    draw_status(frame, chunks[2], state);
    if state.mode == Mode::Settings {
        draw_settings_popup(frame, area, state);
    }
}

fn draw_messages(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let block = widgets::log_block("AI Agent Chat");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines: Vec<Line> = Vec::new();
    for msg in &state.messages {
        if msg.role == "system" {
            continue;
        }
        let (label, color) = match msg.role.as_str() {
            "user" => ("You", theme::SUCCESS),
            "assistant" => ("AI", theme::PRIMARY),
            "tool" => ("Tool", theme::MUTED),
            _ => ("?", theme::MUTED),
        };
        let prefix = Span::styled(format!("{}: ", label), Style::default().fg(color).add_modifier(Modifier::BOLD));
        let text = Span::raw(&msg.text);
        lines.push(Line::from(vec![prefix, text]));
    }
    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((state.scroll, 0));
    frame.render_widget(para, inner);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(state.messages.len().saturating_sub(inner.height as usize))
        .position(state.scroll as usize);
    frame.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
}

fn draw_input(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let block = widgets::form_block("Message (Enter to send, Esc back, s settings)");
    let text = if state.busy { "Thinking...".to_string() } else { state.input.display() };
    let para = Paragraph::new(text).block(block);
    frame.render_widget(para, area);
}

fn draw_status(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let text = if !state.error.is_empty() {
        Line::from(Span::styled(&state.error, theme::error_style()))
    } else if state.provider.is_none() {
        Line::from(Span::styled("No provider configured. Press 's'.", theme::error_style()))
    } else if state.settings_saved {
        Line::from(Span::styled("Settings saved.", theme::SUCCESS))
    } else {
        Line::from(Span::styled("Ctrl+C quit | ↑/↓ scroll | s settings", theme::hint_style()))
    };
    frame.render_widget(Paragraph::new(text), area);
}

fn draw_settings_popup(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let popup = widgets::centered_fixed(70, 16, area);
    let block = widgets::form_block("AI Provider Settings (Tab/Shift+Tab, Enter save, Esc close)");
    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let labels = ["Provider", "Model", "Endpoint", "API Key"];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2); 4])
        .split(inner);
    for (i, (chunk, label)) in chunks.iter().zip(labels.iter()).enumerate() {
        let style = if i == state.settings_focus {
            theme::focused_field_style()
        } else {
            Style::default()
        };
        let display = state.settings_fields[i].display();
        let para = Paragraph::new(format!("{}: {}", label, display)).style(style);
        frame.render_widget(para, *chunk);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_parsing() {
        assert_eq!(kind_from_str("anthropic"), ProviderKind::Anthropic);
        assert_eq!(kind_from_str("openai"), ProviderKind::OpenAiCompatible);
        assert_eq!(kind_from_str("gemini"), ProviderKind::Gemini);
        assert_eq!(kind_from_str("ollama"), ProviderKind::Ollama);
    }
}
