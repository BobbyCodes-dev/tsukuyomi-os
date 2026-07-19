use anyhow::Result;

use super::ai_client::{ToolCall, ToolDefinition};

pub fn all_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "list_os_apps".to_string(),
            description: "List the applications available inside Tsukuyomi OS.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        ToolDefinition {
            name: "open_app".to_string(),
            description: "Open an app inside Tsukuyomi OS. Use 'name' from list_os_apps.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "App identifier, e.g. vault or settings" }
                },
                "required": ["name"]
            }),
        },
        ToolDefinition {
            name: "describe_screen".to_string(),
            description: "Describe the current screen the user is on.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
    ]
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub name: String,
    pub output: String,
}

#[derive(Debug)]
pub struct DispatcherState {
    pub current_screen: String,
    pub available_apps: Vec<String>,
    pub requested_action: Option<crate::app::Action>,
}

impl DispatcherState {
    pub fn new() -> Self {
        Self {
            current_screen: "desktop".to_string(),
            available_apps: vec![
                "desktop".to_string(),
                "vault".to_string(),
                "settings".to_string(),
                "engagements".to_string(),
                "assets".to_string(),
                "scan_request".to_string(),
                "findings".to_string(),
                "evidence".to_string(),
                "cve".to_string(),
                "network".to_string(),
                "health".to_string(),
                "osint_notes".to_string(),
                "remote_support".to_string(),
                "backups".to_string(),
                "connect".to_string(),
                "voidaccess".to_string(),
                "aimap".to_string(),
                "netryx".to_string(),
                "phoneinfoga".to_string(),
                "fawkes".to_string(),
                "paramspider".to_string(),
                "photon".to_string(),
                "onionshare".to_string(),
                "reconftw".to_string(),
                "canarytokens".to_string(),
                "john".to_string(),
                "hashcat".to_string(),
                "hydra".to_string(),
                "hashid".to_string(),
                "crunch".to_string(),
            ],
            requested_action: None,
        }
    }
}

pub fn dispatch(state: &mut DispatcherState, call: &ToolCall) -> Result<ToolResult> {
    let args = &call.arguments;
    let output = match call.name.as_str() {
        "list_os_apps" => {
            let apps: Vec<String> = state.available_apps.iter().map(|a| format!("- {}", a)).collect();
            apps.join("\n")
        }
        "open_app" => {
            let name = args["name"].as_str().unwrap_or("").to_lowercase();
            let action = match name.as_str() {
                "vault" => Some(crate::app::Action::ToVault),
                "settings" => Some(crate::app::Action::ToSettings),
                "engagements" => Some(crate::app::Action::ToEngagements),
                "assets" => Some(crate::app::Action::ToAssets),
                "scan_request" => Some(crate::app::Action::ToScanRequest),
                "findings" => Some(crate::app::Action::ToFindings),
                "evidence" => Some(crate::app::Action::ToEvidence),
                "cve" => Some(crate::app::Action::ToCve),
                "network" => Some(crate::app::Action::ToNetwork),
                "health" => Some(crate::app::Action::ToHealth),
                "osint_notes" => Some(crate::app::Action::ToOsintNotes),
                "remote_support" => Some(crate::app::Action::ToRemoteSupport),
                "connect" => Some(crate::app::Action::ToConnect),
                "voidaccess" => Some(crate::app::Action::ToVoidAccess),
                "aimap" => Some(crate::app::Action::ToAimap),
                "netryx" => Some(crate::app::Action::ToNetryx),
                "phoneinfoga" => Some(crate::app::Action::ToPhoneInfoga),
                "fawkes" => Some(crate::app::Action::ToFawkes),
                "paramspider" => Some(crate::app::Action::ToParamSpider),
                "photon" => Some(crate::app::Action::ToPhoton),
                "onionshare" => Some(crate::app::Action::ToOnionShare),
                "reconftw" => Some(crate::app::Action::ToReconFtw),
                "canarytokens" => Some(crate::app::Action::ToCanarytokens),
                "john" => Some(crate::app::Action::ToJohn),
                "hashcat" => Some(crate::app::Action::ToHashcat),
                "hydra" => Some(crate::app::Action::ToHydra),
                "hashid" => Some(crate::app::Action::ToHashid),
                "crunch" => Some(crate::app::Action::ToCrunch),
                "desktop" => Some(crate::app::Action::Back),
                _ => None,
            };
            if let Some(a) = action {
                state.requested_action = Some(a);
                format!("Opening {}...", name)
            } else {
                format!("Unknown app: {}", name)
            }
        }
        "describe_screen" => format!("Current screen: {}", state.current_screen),
        _ => format!("Tool '{}' not implemented.", call.name),
    };
    Ok(ToolResult {
        tool_call_id: call.id.clone(),
        name: call.name.clone(),
        output,
    })
}

#[allow(dead_code)]
pub fn tool_results_to_messages(results: &[ToolResult]) -> Vec<super::ai_client::ChatMessage> {
    results
        .iter()
        .map(|r| super::ai_client::ChatMessage {
            role: "tool".to_string(),
            content: format!("{}: {}", r.name, r.output),
        })
        .collect()
}
