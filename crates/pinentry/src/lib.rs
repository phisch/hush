use std::io::{self, BufRead, Write};

use zeroize::Zeroizing;

use client_request::ClientRequest;
use error::AssuanError;
use option::PinentryOption;
use response::Response;

pub mod client_request;
pub mod error;
pub mod option;
pub mod response;

#[derive(Debug, Default, Clone)]
pub struct Settings {
    pub timeout: Option<u32>,
    pub description: Option<String>,
    pub prompt: Option<String>,
    pub title: Option<String>,
    pub ok_label: Option<String>,
    pub cancel_label: Option<String>,
    pub not_ok_label: Option<String>,
    pub error: Option<String>,
    pub repeat: Option<String>,
    pub repeat_error: Option<String>,
    pub quality_bar: Option<String>,
    pub quality_bar_tooltip: Option<String>,
    pub generate_pin: Option<String>,
    pub generate_pin_tooltip: Option<String>,
    pub key_info: Option<String>,
    pub formatted_passphrase: bool,
    pub allow_external_cache: bool,

    pub default_ok: Option<String>,
    pub default_cancel: Option<String>,
    pub default_prompt: Option<String>,
    pub ttyname: Option<String>,
    pub ttytype: Option<String>,
    pub lc_ctype: Option<String>,
    pub lc_messages: Option<String>,
}

pub enum PinOutcome {
    Entered(Zeroizing<String>),
    Cancelled,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfirmOutcome {
    Yes,
    No,
    Cancelled,
}

pub trait Frontend {
    fn get_pin(&mut self, settings: &Settings) -> PinOutcome;
    fn confirm(&mut self, settings: &Settings, one_button: bool) -> ConfirmOutcome;
    fn message(&mut self, settings: &Settings);
}

pub struct Pinentry<R, W, F> {
    reader: R,
    writer: W,
    frontend: F,
    settings: Settings,
    should_quit: bool,
}

impl<R, W, F> Pinentry<R, W, F>
where
    R: BufRead,
    W: Write,
    F: Frontend,
{
    pub fn new(reader: R, writer: W, frontend: F) -> Self {
        Pinentry {
            reader,
            writer,
            frontend,
            settings: Settings::default(),
            should_quit: false,
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.send(Response::Ok(Some("Pleased to meet you".into())))?;

        let mut line = String::new();
        loop {
            line.clear();
            if self.reader.read_line(&mut line)? == 0 {
                break;
            }

            if line.trim().is_empty() {
                continue;
            }

            for response in self.handle(ClientRequest::parse(&line)) {
                self.send(response)?;
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn send(&mut self, response: Response) -> io::Result<()> {
        writeln!(self.writer, "{response}")?;
        self.writer.flush()
    }

    fn handle(&mut self, request: Option<ClientRequest>) -> Vec<Response> {
        let Some(request) = request else {
            return vec![Response::Error(AssuanError::UnknownIpcCommand)];
        };

        match request {
            ClientRequest::Bye | ClientRequest::Quit => {
                self.should_quit = true;
                vec![Response::Ok(Some("closing connection".into()))]
            }
            ClientRequest::Reset => {
                self.settings = Settings::default();
                ok()
            }
            ClientRequest::Nop
            | ClientRequest::Cancel
            | ClientRequest::End
            | ClientRequest::Help => ok(),
            ClientRequest::Auth => not_implemented(),
            ClientRequest::Option(option) => {
                self.apply_option(option);
                ok()
            }

            ClientRequest::SetTimeout(timeout) => {
                self.settings.timeout = (timeout != 0).then_some(timeout);
                ok()
            }
            ClientRequest::SetDescription(text) => set(&mut self.settings.description, text),
            ClientRequest::SetPrompt(text) => set(&mut self.settings.prompt, text),
            ClientRequest::SetTitle(text) => set(&mut self.settings.title, text),
            ClientRequest::SetOk(text) => set(&mut self.settings.ok_label, text),
            ClientRequest::SetCancel(text) => set(&mut self.settings.cancel_label, text),
            ClientRequest::SetNotOk(text) => set(&mut self.settings.not_ok_label, text),
            ClientRequest::SetError(text) => set(&mut self.settings.error, text),
            ClientRequest::SetRepeat(label) => {
                self.settings.repeat = Some(label);
                ok()
            }
            ClientRequest::SetRepeatError(text) => set(&mut self.settings.repeat_error, text),
            ClientRequest::SetQualityBar(label) => {
                self.settings.quality_bar = Some(label);
                ok()
            }
            ClientRequest::SetQualityBarTooltip(text) => {
                set(&mut self.settings.quality_bar_tooltip, text)
            }
            ClientRequest::SetGenpin(label) => {
                self.settings.generate_pin = Some(label);
                ok()
            }
            ClientRequest::SetGenpinTooltip(text) => {
                set(&mut self.settings.generate_pin_tooltip, text)
            }
            ClientRequest::SetKeyInfo(text) => set(&mut self.settings.key_info, text),

            ClientRequest::GetPin => self.get_pin(),
            ClientRequest::Confirm { one_button } => self.confirm(one_button),
            ClientRequest::Message => {
                self.frontend.message(&self.settings);
                self.settings.error = None;
                ok()
            }
            ClientRequest::GetInfo(what) => self.get_info(&what),
            ClientRequest::ClearPassphrase(_) => ok(),
        }
    }

    fn apply_option(&mut self, option: PinentryOption) {
        match option {
            PinentryOption::FormattedPassphrase => self.settings.formatted_passphrase = true,
            PinentryOption::AllowExternalPasswordCache => self.settings.allow_external_cache = true,
            PinentryOption::TtyType(value) => self.settings.ttytype = Some(value),
            PinentryOption::TtyName(value) => self.settings.ttyname = Some(value),
            PinentryOption::LcCtype(value) => self.settings.lc_ctype = Some(value),
            PinentryOption::LcMessages(value) => self.settings.lc_messages = Some(value),
            PinentryOption::DefaultOk(value) => self.settings.default_ok = Some(value),
            PinentryOption::DefaultCancel(value) => self.settings.default_cancel = Some(value),
            PinentryOption::DefaultPrompt(value) => self.settings.default_prompt = Some(value),
            PinentryOption::Other(_) => {}
        }
    }

    fn get_pin(&mut self) -> Vec<Response> {
        let outcome = self.frontend.get_pin(&self.settings);
        let repeated = self.settings.repeat.is_some();
        self.settings.error = None;

        match outcome {
            PinOutcome::Entered(pin) => {
                let mut responses = Vec::new();
                if repeated {
                    responses.push(Response::Status("PIN_REPEATED".into()));
                }
                if !pin.is_empty() {
                    responses.push(Response::Data(pin.as_str().to_owned()));
                }
                responses.push(Response::Ok(None));
                responses
            }
            PinOutcome::Cancelled => vec![Response::Error(AssuanError::Canceled)],
        }
    }

    fn confirm(&mut self, one_button: bool) -> Vec<Response> {
        let outcome = self.frontend.confirm(&self.settings, one_button);
        self.settings.error = None;

        match outcome {
            ConfirmOutcome::Yes => ok(),
            ConfirmOutcome::No => vec![Response::Error(AssuanError::NotConfirmed)],
            ConfirmOutcome::Cancelled => vec![Response::Error(AssuanError::Canceled)],
        }
    }

    fn get_info(&self, what: &str) -> Vec<Response> {
        let data = match what {
            "version" => env!("CARGO_PKG_VERSION").to_string(),
            "pid" => std::process::id().to_string(),
            "flavor" => "wayland".to_string(),
            "ttyinfo" => {
                let dash = "-".to_string();
                let ttyname = self.settings.ttyname.as_ref().unwrap_or(&dash);
                let ttytype = self.settings.ttytype.as_ref().unwrap_or(&dash);
                format!("{ttyname} {ttytype} - - - -")
            }
            _ => return ok(),
        };
        vec![Response::Data(data), Response::Ok(None)]
    }
}

fn ok() -> Vec<Response> {
    vec![Response::Ok(None)]
}

fn not_implemented() -> Vec<Response> {
    vec![Response::Error(AssuanError::NotImplemented)]
}

fn set(field: &mut Option<String>, value: String) -> Vec<Response> {
    *field = Some(value);
    ok()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[derive(Default)]
    struct MockFrontend {
        pin_outcome: Option<PinOutcome>,
        confirm_outcome: Option<ConfirmOutcome>,
        seen: Option<Settings>,
        messages: usize,
    }

    impl Frontend for MockFrontend {
        fn get_pin(&mut self, settings: &Settings) -> PinOutcome {
            self.seen = Some(settings.clone());
            self.pin_outcome
                .take()
                .unwrap_or(PinOutcome::Entered(Zeroizing::new(String::new())))
        }

        fn confirm(&mut self, settings: &Settings, _one_button: bool) -> ConfirmOutcome {
            self.seen = Some(settings.clone());
            self.confirm_outcome.take().unwrap_or(ConfirmOutcome::Yes)
        }

        fn message(&mut self, settings: &Settings) {
            self.seen = Some(settings.clone());
            self.messages += 1;
        }
    }

    #[allow(clippy::type_complexity)]
    fn run_with(
        frontend: MockFrontend,
        input: &[&str],
    ) -> (
        Vec<String>,
        Pinentry<Cursor<Vec<u8>>, Vec<u8>, MockFrontend>,
    ) {
        let reader = Cursor::new((input.join("\n") + "\n").into_bytes());
        let mut pinentry = Pinentry::new(reader, Vec::new(), frontend);
        pinentry.run().unwrap();

        let output = String::from_utf8(pinentry.writer.clone()).unwrap();
        let lines = output.lines().map(str::to_string).collect();
        (lines, pinentry)
    }

    #[test]
    fn greets_then_acknowledges_settings_and_closes() {
        let (lines, _) = run_with(MockFrontend::default(), &["SETDESC hello", "BYE"]);
        assert_eq!(
            lines,
            vec!["OK Pleased to meet you", "OK", "OK closing connection"]
        );
    }

    #[test]
    fn collects_settings_for_the_dialog() {
        let (_, pinentry) = run_with(
            MockFrontend::default(),
            &[
                "SETTITLE Authentication",
                "SETDESC Please unlock the key",
                "SETPROMPT Passphrase:",
                "OPTION ttyname=/dev/pts/2",
                "GETPIN",
            ],
        );
        let seen = pinentry.frontend.seen.unwrap();
        assert_eq!(seen.title.as_deref(), Some("Authentication"));
        assert_eq!(seen.description.as_deref(), Some("Please unlock the key"));
        assert_eq!(seen.prompt.as_deref(), Some("Passphrase:"));
        assert_eq!(seen.ttyname.as_deref(), Some("/dev/pts/2"));
    }

    #[test]
    fn getpin_returns_data_then_ok() {
        let frontend = MockFrontend {
            pin_outcome: Some(PinOutcome::Entered(Zeroizing::new("hunter2".into()))),
            ..Default::default()
        };
        let (lines, _) = run_with(frontend, &["GETPIN", "BYE"]);
        assert_eq!(lines[1], "D hunter2");
        assert_eq!(lines[2], "OK");
    }

    #[test]
    fn getpin_with_empty_passphrase_returns_only_ok() {
        let frontend = MockFrontend {
            pin_outcome: Some(PinOutcome::Entered(Zeroizing::new(String::new()))),
            ..Default::default()
        };
        let (lines, _) = run_with(frontend, &["GETPIN", "BYE"]);
        assert_eq!(
            lines,
            vec!["OK Pleased to meet you", "OK", "OK closing connection"]
        );
    }

    #[test]
    fn getpin_emits_pin_repeated_status_when_repeat_is_set() {
        let frontend = MockFrontend {
            pin_outcome: Some(PinOutcome::Entered(Zeroizing::new("secret".into()))),
            ..Default::default()
        };
        let (lines, _) = run_with(frontend, &["SETREPEAT Repeat:", "GETPIN", "BYE"]);
        assert_eq!(lines[1], "OK");
        assert_eq!(lines[2], "S PIN_REPEATED");
        assert_eq!(lines[3], "D secret");
        assert_eq!(lines[4], "OK");
    }

    #[test]
    fn cancelled_getpin_returns_canceled_error() {
        let frontend = MockFrontend {
            pin_outcome: Some(PinOutcome::Cancelled),
            ..Default::default()
        };
        let (lines, _) = run_with(frontend, &["GETPIN", "BYE"]);
        assert_eq!(
            lines[1],
            "ERR 536871011 Operation cancelled <User defined source 1>"
        );
    }

    #[test]
    fn confirm_maps_outcomes_to_protocol_responses() {
        for (outcome, expected) in [
            (ConfirmOutcome::Yes, "OK"),
            (
                ConfirmOutcome::No,
                "ERR 536871026 Not confirmed <User defined source 1>",
            ),
            (
                ConfirmOutcome::Cancelled,
                "ERR 536871011 Operation cancelled <User defined source 1>",
            ),
        ] {
            let frontend = MockFrontend {
                confirm_outcome: Some(outcome),
                ..Default::default()
            };
            let (lines, _) = run_with(frontend, &["CONFIRM", "BYE"]);
            assert_eq!(lines[1], expected);
        }
    }

    #[test]
    fn message_shows_a_dialog_and_acks() {
        let (lines, pinentry) = run_with(MockFrontend::default(), &["MESSAGE", "BYE"]);
        assert_eq!(lines[1], "OK");
        assert_eq!(pinentry.frontend.messages, 1);
    }

    #[test]
    fn seterror_is_cleared_after_a_dialog() {
        let frontend = MockFrontend {
            pin_outcome: Some(PinOutcome::Entered(Zeroizing::new("x".into()))),
            ..Default::default()
        };
        let (_, pinentry) = run_with(frontend, &["SETERROR oops", "GETPIN"]);
        assert_eq!(pinentry.settings.error, None);
    }

    #[test]
    fn reset_clears_settings() {
        let (_, pinentry) = run_with(
            MockFrontend::default(),
            &["SETDESC hello", "SETPROMPT pw", "RESET"],
        );
        assert!(pinentry.settings.description.is_none());
        assert!(pinentry.settings.prompt.is_none());
    }

    #[test]
    fn getinfo_version_returns_the_crate_version() {
        let (lines, _) = run_with(MockFrontend::default(), &["GETINFO version", "BYE"]);
        assert_eq!(lines[1], format!("D {}", env!("CARGO_PKG_VERSION")));
        assert_eq!(lines[2], "OK");
    }

    #[test]
    fn unknown_command_reports_unknown_ipc_command() {
        let (lines, _) = run_with(MockFrontend::default(), &["FROBNICATE", "BYE"]);
        assert_eq!(
            lines[1],
            "ERR 536871187 Unknown IPC command <User defined source 1>"
        );
    }

    #[test]
    fn blank_lines_are_ignored() {
        let (lines, _) = run_with(MockFrontend::default(), &["", "   ", "BYE"]);
        assert_eq!(
            lines,
            vec!["OK Pleased to meet you", "OK closing connection"]
        );
    }
}
