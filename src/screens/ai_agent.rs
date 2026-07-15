use std::sync::mpsc::{self, Receiver};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, Wrap};
use ratatui::Frame;

use crate::app::Action;
use crate::store::ai::{AiProvider, ProviderKind, load_provider};
use crate::store::ai_client::{ChatMessage, AiResponse};
use crate::store::ai_tools::{all_tools, dispatch, DispatcherState};
use crate::store::vault::{VaultKey, get_entry_by_name};
use crate::ui::{theme, widgets};

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
    messages: Vec<Message>,
    input: widgets::TextField,
    scroll: u16,
    busy: bool,
    error: String,
    provider: Option<AiProvider>,
    dispatcher: DispatcherState,
    chat_rx: Option<Receiver<ChatOutcome>>,
}

enum ChatOutcome {
    Response(AiResponse),
    Error(String),
}

impl AiAgentState {
    pub fn new(user_id: i64, vault_key: VaultKey) -> Self {
        let provider = load_provider(user_id).ok().flatten();
        let mut state = Self {
            user_id,
            vault_key,
            messages: vec![Message {
                role: "system".to_string(),
                text: "You are Tsukuyomi AI, an assistant embedded in Tsukuyomi OS, a local security-focused terminal desktop. \
Answer in plain, direct sentences. Do not format answers as bullet points or numbered lists unless the user explicitly asks for a list. \
Saved credentials (usernames, passwords, notes) live in the Credential Vault app, encrypted with AES-256-GCM using a key derived from the \
user's login password; nothing is stored in plaintext. You can open apps and describe the current screen using your tools \
(list_os_apps, open_app, describe_screen) — use them when a question is about what's on this OS or where something lives, \
rather than guessing.".to_string(),
            }],
            input: widgets::TextField::new(),
            scroll: 0,
            busy: false,
            error: String::new(),
            provider,
            dispatcher: DispatcherState::new(),
            chat_rx: None,
        };
        if state.provider.is_none() {
            state.error = "No AI provider configured. Open Settings from the desktop to set provider, model, endpoint and API key.".to_string();
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
        if api_key.is_empty() && provider.kind != ProviderKind::Ollama {
            return None;
        }
        Some((provider, api_key))
    }

    pub fn poll(&mut self) {
        let Some(rx) = &self.chat_rx else { return };
        match rx.try_recv() {
            Ok(ChatOutcome::Response(resp)) => {
                self.busy = false;
                self.chat_rx = None;
                handle_response(self, resp);
            }
            Ok(ChatOutcome::Error(e)) => {
                self.busy = false;
                self.chat_rx = None;
                self.error = e;
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.busy = false;
                self.chat_rx = None;
                self.error = "AI request thread ended unexpectedly.".to_string();
            }
        }
    }
}

fn vault_entry_name(provider: &AiProvider) -> String {
    format!("ai-provider-{:?}", provider.kind).to_lowercase()
}

pub fn handle_key(state: &mut AiAgentState, key: KeyEvent) -> Action {
    if key.code == KeyCode::Esc {
        return Action::Back;
    }
    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => Action::Quit,
        KeyCode::Enter => {
            let text = state.input.value.trim().to_string();
            if !text.is_empty() && !state.busy {
                state.input.value.clear();
                state.messages.push(Message { role: "user".to_string(), text });
                state.scroll = u16::MAX;
                submit_chat(state);
            }
            Action::None
        }
        KeyCode::Char(c) => {
            state.input.push_char(c);
            Action::None
        }
        KeyCode::Backspace => {
            state.input.backspace();
            Action::None
        }
        KeyCode::Up => {
            state.scroll = state.scroll.saturating_sub(1);
            Action::None
        }
        KeyCode::Down => {
            state.scroll = state.scroll.saturating_add(1);
            Action::None
        }
        _ => Action::None,
    }
}

fn submit_chat(state: &mut AiAgentState) {
    let (provider, api_key) = match state.current_provider() {
        Some((provider, api_key)) => (provider.clone(), api_key),
        None => {
            state.error = "No AI provider configured. Open Settings from the desktop to configure.".to_string();
            state.messages.push(Message {
                role: "assistant".to_string(),
                text: "AI provider is not configured. Open Settings from the desktop to set provider, model, endpoint, and API key.".to_string(),
            });
            return;
        }
    };
    let client = crate::store::ai_client::build_client(&provider, &api_key);
    let tools = all_tools();
    let chat_messages: Vec<ChatMessage> = state
        .messages
        .iter()
        .filter(|m| !m.is_tool())
        .map(|m| ChatMessage { role: m.role.clone(), content: m.text.clone() })
        .collect();

    let (tx, rx) = mpsc::channel();
    state.chat_rx = Some(rx);
    state.busy = true;
    state.error.clear();
    std::thread::spawn(move || {
        let outcome = match tokio::runtime::Runtime::new() {
            Ok(rt) => match rt.block_on(client.complete(&chat_messages, &tools)) {
                Ok(resp) => ChatOutcome::Response(resp),
                Err(e) => ChatOutcome::Error(format!("AI request failed: {e}")),
            },
            Err(e) => ChatOutcome::Error(format!("AI request failed: {e}")),
        };
        let _ = tx.send(outcome);
    });
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
}

fn wrapped_row_count(text: &str, width: u16) -> usize {
    let width = (width as usize).max(1);
    let mut rows = 0usize;
    for raw_line in text.split('\n') {
        let mut current = 0usize;
        let mut any_word = false;
        for word in raw_line.split_whitespace() {
            any_word = true;
            let mut word_len = word.chars().count();
            let fits_current_row = current > 0 && current + 1 + word_len <= width;
            if fits_current_row {
                current += 1 + word_len;
                continue;
            }
            rows += 1;
            while word_len > width {
                rows += 1;
                word_len -= width;
            }
            current = word_len;
        }
        if !any_word {
            rows += 1;
        }
    }
    rows.max(1)
}

fn draw_messages(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let block = widgets::log_block("AI Agent Chat");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let mut lines: Vec<Line> = Vec::new();
    let mut total_rows = 0usize;
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
        let prefix_str = format!("{label}: ");
        let prefix = Span::styled(prefix_str.clone(), Style::default().fg(color).add_modifier(Modifier::BOLD));
        let text = Span::raw(&msg.text);
        lines.push(Line::from(vec![prefix, text]));
        total_rows += wrapped_row_count(&format!("{prefix_str}{}", msg.text), inner.width);
    }
    let max_scroll = (total_rows as u16).saturating_sub(inner.height);
    let scroll = state.scroll.min(max_scroll);
    let para = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(para, inner);
    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    let mut scrollbar_state = ratatui::widgets::ScrollbarState::new(max_scroll as usize).position(scroll as usize);
    frame.render_stateful_widget(scrollbar, inner, &mut scrollbar_state);
}

fn draw_input(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let block = widgets::form_block("Message (Enter to send, Esc back)");
    let text = if state.busy { "Thinking...".to_string() } else { state.input.display() };
    let para = Paragraph::new(text).block(block);
    frame.render_widget(para, area);
}

fn draw_status(frame: &mut Frame, area: Rect, state: &AiAgentState) {
    let text = if !state.error.is_empty() {
        Line::from(Span::styled(&state.error, theme::error_style()))
    } else if state.provider.is_none() {
        Line::from(Span::styled("No provider configured. Open Settings from the desktop.", theme::error_style()))
    } else {
        Line::from(Span::styled("Ctrl+C quit | ↑/↓ scroll | Esc back", theme::hint_style()))
    };
    frame.render_widget(Paragraph::new(text), area);
}
