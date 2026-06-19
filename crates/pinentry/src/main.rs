use std::io::{stdin, stdout};

use dialog::{run_dialog, DialogConfig, DialogKind, DialogResult};
use pinentry::{ConfirmOutcome, Frontend, PinOutcome, Pinentry, Settings};

struct LayerShell;

impl LayerShell {
    fn config(settings: &Settings, kind: DialogKind) -> DialogConfig {
        let default_heading = match kind {
            DialogKind::Pin => "Unlock your key",
            DialogKind::Confirm { .. } => "Please confirm",
            DialogKind::Message => "Notice",
        };

        DialogConfig {
            heading: non_empty(settings.title.clone())
                .map(|t| strip_accel(&t))
                .unwrap_or_else(|| default_heading.to_string()),
            description: non_empty(settings.description.clone()),
            error: non_empty(settings.error.clone()),
            placeholder: non_empty(settings.prompt.clone())
                .or_else(|| non_empty(settings.default_prompt.clone()))
                .map(|p| strip_accel(p.trim_end_matches(':')))
                .unwrap_or_else(|| "Enter PIN".to_string()),
            ok_label: clean_label(
                settings
                    .ok_label
                    .clone()
                    .or_else(|| settings.default_ok.clone()),
                match kind {
                    DialogKind::Pin => "Unlock",
                    _ => "OK",
                },
            ),
            cancel_label: clean_label(
                settings
                    .cancel_label
                    .clone()
                    .or_else(|| settings.default_cancel.clone()),
                "Cancel",
            ),
            not_ok_label: non_empty(settings.not_ok_label.clone()).map(|l| strip_accel(&l)),
            repeat_label: settings.repeat.clone().map(|l| strip_accel(&l)),
            repeat_error: settings.repeat_error.clone().unwrap_or_default(),
            quality_bar: settings.quality_bar.is_some(),
            choice_label: None,
            choice: false,
            kind,
        }
    }
}

impl Frontend for LayerShell {
    fn get_pin(&mut self, settings: &Settings) -> PinOutcome {
        match run_dialog(Self::config(settings, DialogKind::Pin)) {
            DialogResult::Pin { secret, .. } => PinOutcome::Entered(secret),
            _ => PinOutcome::Cancelled,
        }
    }

    fn confirm(&mut self, settings: &Settings, one_button: bool) -> ConfirmOutcome {
        match run_dialog(Self::config(settings, DialogKind::Confirm { one_button })) {
            DialogResult::Confirmed { .. } => ConfirmOutcome::Yes,
            DialogResult::Declined => ConfirmOutcome::No,
            _ => ConfirmOutcome::Cancelled,
        }
    }

    fn message(&mut self, settings: &Settings) {
        let _ = run_dialog(Self::config(settings, DialogKind::Message));
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.trim().is_empty())
}

fn clean_label(label: Option<String>, default: &str) -> String {
    match non_empty(label) {
        Some(label) => strip_accel(&label),
        None => default.to_string(),
    }
}

fn strip_accel(label: &str) -> String {
    label.replace('_', "")
}

fn main() {
    // No core dumps/ptrace: the passphrase lives in this process's memory.
    hardening::forbid_dumps();

    let mut pinentry = Pinentry::new(stdin().lock(), stdout().lock(), LayerShell);

    if let Err(error) = pinentry.run() {
        eprintln!("hush-pinentry: {error}");
        std::process::exit(1);
    }
}
