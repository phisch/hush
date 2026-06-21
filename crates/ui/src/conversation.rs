use std::io::{BufRead, Write};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use gpui::{div, prelude::*, px, App, Application, Context, Entity, FocusHandle, Window};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::chrome::{backdrop, banner, button, card, header, hint_line, info_block};
use crate::fade::Fade;
use theme::theme;
use crate::field::Field;
use crate::{arm_watchdog, layer_window, Cancel, Icons, Submit};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Update {
    Start { title: String, message: String },
    Info { text: String },
    Error { text: String },
    Prompt { echo_on: bool, label: String },
    Done { success: bool },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    Response { secret: String },
    Cancel,
}

struct Convo {
    title: String,
    body: String,
    info: Vec<String>,
    error: Option<String>,
    prompting: bool,
    field: Option<Entity<Field>>,
    pending_focus: bool,
    focus: FocusHandle,
    fade: Fade,
}

impl Convo {
    fn new(cx: &mut Context<Self>, rx: Receiver<Option<Update>>) -> Self {
        cx.spawn(async move |this, cx| loop {
            match rx.try_recv() {
                Ok(Some(update)) => {
                    if this
                        .update(cx, |convo, cx| convo.apply(update, cx))
                        .is_err()
                    {
                        break;
                    }
                }
                Ok(None) => {
                    let _ = this.update(cx, |convo, cx| convo.finish(cx));
                    break;
                }
                Err(TryRecvError::Empty) => {
                    cx.background_executor()
                        .timer(Duration::from_millis(20))
                        .await
                }
                Err(TryRecvError::Disconnected) => break,
            }
        })
        .detach();

        Self {
            title: String::new(),
            body: String::new(),
            info: Vec::new(),
            error: None,
            prompting: false,
            field: None,
            pending_focus: false,
            focus: cx.focus_handle(),
            fade: Fade::default(),
        }
    }

    fn apply(&mut self, update: Update, cx: &mut Context<Self>) {
        match update {
            Update::Start { title, message } => {
                self.title = title;
                self.body = message;
            }
            Update::Info { text } => self.info.push(text),
            Update::Error { text } => self.error = Some(text),
            Update::Prompt { echo_on, label } => {
                let field = cx.new(|cx| Field::new(cx, !echo_on, label, px(0.)));
                self.field = Some(field);
                self.prompting = true;
                self.pending_focus = true;
            }
            Update::Done { .. } => return self.finish(cx),
        }
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.prompting) {
            return;
        }
        let secret = self
            .field
            .as_ref()
            .map(|field| field.update(cx, |f, _| f.take_value()))
            .unwrap_or_default();
        emit(&Reply::Response {
            secret: secret.to_string(),
        });
        self.field = None;
        self.error = None;
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        emit(&Reply::Cancel);
        self.finish(cx);
    }

    fn finish(&self, cx: &mut Context<Self>) {
        cx.quit();
    }
}

impl Render for Convo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pending_focus {
            if let Some(field) = &self.field {
                window.focus(&field.read(cx).focus_handle());
            }
            self.pending_focus = false;
        }

        let opacity = self.fade.opacity(window);
        let mut body: Vec<gpui::AnyElement> = vec![header(&self.title).into_any_element()];

        if let Some(info) = info_block(
            self.body
                .lines()
                .chain(self.info.iter().map(String::as_str)),
        ) {
            body.push(info);
        }
        if let Some(error) = &self.error {
            body.push(banner(error).into_any_element());
        }
        if let Some(field) = &self.field {
            body.push(field.clone().into_any_element());
        }
        body.push(self.footer(cx));

        backdrop()
            .track_focus(&self.focus)
            .key_context("Convo")
            .on_action(cx.listener(|this, _: &Submit, _, cx| this.submit(cx)))
            .on_action(cx.listener(|this, _: &Cancel, _, cx| this.cancel(cx)))
            .opacity(opacity)
            .child(card(body))
    }
}

impl Convo {
    fn footer(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let prompting = self.prompting;
        let items: &[(&str, &str)] = if prompting {
            &[("\u{21B5}", "authenticate"), ("Esc", "cancel")]
        } else {
            &[("Esc", "cancel")]
        };

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .w_full()
            .child(hint_line(items))
            .child(div().flex_1())
            .child(
                button("cancel", "Cancel".to_string(), &theme().cancel)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
            );

        if prompting {
            row = row.child(
                button("authenticate", "Authenticate".to_string(), &theme().confirm)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            );
        }

        row.into_any_element()
    }
}

fn emit(message: &Reply) {
    if let Ok(line) = serde_json::to_string(message) {
        let line = Zeroizing::new(line);
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{}", &*line);
        let _ = out.flush();
    }
}

pub fn run_conversation() {
    Application::new()
        .with_assets(Icons)
        .run(move |cx: &mut App| {
            cx.bind_keys([
                gpui::KeyBinding::new("enter", Submit, Some("Convo")),
                gpui::KeyBinding::new("escape", Cancel, Some("Convo")),
            ]);

            let (tx, rx) = std::sync::mpsc::channel::<Option<Update>>();
            std::thread::spawn(move || {
                for line in std::io::stdin().lock().lines() {
                    let Ok(line) = line else { break };
                    if let Ok(update) = serde_json::from_str::<Update>(line.trim()) {
                        if tx.send(Some(update)).is_err() {
                            return;
                        }
                    }
                }
                let _ = tx.send(None);
            });

            let mut rx = Some(rx);
            cx.spawn(async move |cx| loop {
                match rx.as_ref().unwrap().try_recv() {
                    Ok(Some(first)) => {
                        let rx = rx.take().unwrap();
                        let _ = cx.update(|cx| {
                            arm_watchdog("polkit dialog");
                            cx.open_window(layer_window(), |window, cx| {
                                let convo = cx.new(|cx| Convo::new(cx, rx));
                                convo.update(cx, |convo, cx| convo.apply(first, cx));
                                window.focus(&convo.read(cx).focus);
                                convo
                            })
                            .unwrap();
                        });
                        break;
                    }
                    Ok(None) | Err(TryRecvError::Disconnected) => {
                        let _ = cx.update(|cx| cx.quit());
                        break;
                    }
                    Err(TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(20))
                            .await;
                    }
                }
            })
            .detach();
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_messages_round_trip_one_line() {
        let prompt = Update::Prompt {
            echo_on: false,
            label: "Password: ".into(),
        };
        let line = serde_json::to_string(&prompt).unwrap();
        assert!(!line.contains('\n'));
        assert!(matches!(
            serde_json::from_str::<Update>(&line).unwrap(),
            Update::Prompt { echo_on: false, .. }
        ));
    }

    #[test]
    fn dialog_response_serializes_secret_safely() {
        let line = serde_json::to_string(&Reply::Response {
            secret: "with \"quotes\"\nand newline".into(),
        })
        .unwrap();
        assert!(!line.contains('\n'));
        match serde_json::from_str::<Reply>(&line).unwrap() {
            Reply::Response { secret } => {
                assert_eq!(secret, "with \"quotes\"\nand newline")
            }
            _ => panic!("expected response"),
        }
    }
}
