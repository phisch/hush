use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use base64::prelude::*;
use keyring_prompter::{Cancel, PromptKind, PromptRequest, PromptResponse, Prompter};
use ui::{run_dialog, DialogConfig, DialogKind, DialogResult};
use zeroize::Zeroizing;

const DIALOG_FLAG: &str = "--dialog";
const CHILD_KILL_TIMEOUT: Duration = Duration::from_secs(ui::MAX_LIFETIME.as_secs() + 30);
const POLL: Duration = Duration::from_millis(50);

fn main() {
    if std::env::args().nth(1).as_deref() == Some(DIALOG_FLAG) {
        hardening::forbid_dumps();
        run_dialog_child();
        return;
    }
    hardening::forbid_dumps();
    if let Err(error) = keyring_prompter::run(SubprocessPrompter) {
        eprintln!("psst-keyring: {error}");
        std::process::exit(1);
    }
}

fn run_dialog_child() {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        std::process::exit(1);
    }
    let Ok(config) = serde_json::from_str::<DialogConfig>(&input) else {
        std::process::exit(1);
    };

    let mut out = std::io::stdout().lock();
    match run_dialog(config) {
        DialogResult::Pin { secret, choice } => {
            let encoded = Zeroizing::new(BASE64_STANDARD.encode(secret.as_bytes()));
            let _ = writeln!(out, "pin {} {}", *encoded, choice as u8);
        }
        DialogResult::Confirmed { choice } => {
            let _ = writeln!(out, "yes {}", choice as u8);
        }
        DialogResult::Declined | DialogResult::Cancelled => {
            let _ = writeln!(out, "no");
        }
    }
}

struct SubprocessPrompter;

impl Prompter for SubprocessPrompter {
    fn prompt(&self, request: PromptRequest, cancel: &Cancel) -> PromptResponse {
        let Ok(json) = serde_json::to_string(&dialog_config(&request)) else {
            return PromptResponse::Dismissed;
        };
        show(json, cancel).unwrap_or(PromptResponse::Dismissed)
    }
}

fn show(config_json: String, cancel: &Cancel) -> Option<PromptResponse> {
    let exe = std::env::current_exe().ok()?;
    let mut child = Command::new(exe)
        .arg(DIALOG_FLAG)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;

    {
        let mut stdin = child.stdin.take()?;
        stdin.write_all(config_json.as_bytes()).ok()?;
    }

    let started = Instant::now();
    loop {
        if cancel.is_cancelled() || started.elapsed() > CHILD_KILL_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Some(PromptResponse::Dismissed);
        }
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => return Some(PromptResponse::Dismissed),
        }
    }

    let mut output = Zeroizing::new(String::new());
    child.stdout.take()?.read_to_string(&mut output).ok()?;
    Some(parse_response(&output))
}

fn parse_response(output: &str) -> PromptResponse {
    let mut tokens = output.trim().split(' ');
    match tokens.next() {
        Some("pin") => {
            let Some(encoded) = tokens.next() else {
                return PromptResponse::Dismissed;
            };
            let choice = tokens.next() == Some("1");
            let Ok(bytes) = BASE64_STANDARD.decode(encoded) else {
                return PromptResponse::Dismissed;
            };
            let bytes = Zeroizing::new(bytes);
            match std::str::from_utf8(&bytes) {
                Ok(text) => PromptResponse::Password {
                    secret: Zeroizing::new(text.to_owned()),
                    choice,
                },
                Err(_) => PromptResponse::Dismissed,
            }
        }
        Some("yes") => PromptResponse::Confirmed {
            choice: tokens.next() == Some("1"),
        },
        _ => PromptResponse::Dismissed,
    }
}

fn dialog_config(request: &PromptRequest) -> DialogConfig {
    let confirm = matches!(request.kind, PromptKind::Password { confirm: true });
    let kind = match request.kind {
        PromptKind::Password { .. } => DialogKind::Pin,
        PromptKind::Confirm => DialogKind::Confirm { one_button: false },
    };
    let heading = request.title.clone().unwrap_or_else(|| match request.kind {
        PromptKind::Password { .. } => "Unlock keyring".to_string(),
        PromptKind::Confirm => "Confirm".to_string(),
    });

    DialogConfig {
        kind,
        heading,
        description: request.description.clone(),
        error: request.warning.clone(),
        placeholder: if confirm { "New password" } else { "Password" }.to_string(),
        ok_label: request
            .continue_label
            .clone()
            .unwrap_or_else(|| "Unlock".to_string()),
        cancel_label: request
            .cancel_label
            .clone()
            .unwrap_or_else(|| "Cancel".to_string()),
        not_ok_label: None,
        repeat_label: confirm.then(|| "Confirm password".to_string()),
        repeat_error: "Passwords do not match.".to_string(),
        quality_bar: false,
        choice_label: request.choice_label.clone(),
        choice: request.choice,
    }
}
