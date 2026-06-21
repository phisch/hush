use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{
    actions, div, layer_shell::*, prelude::*, px, svg, AnyElement, App, Application, AssetSource,
    Context, Div, FocusHandle, MouseButton, Result, SharedString, WindowBackgroundAppearance,
    WindowKind, WindowOptions,
};
use serde::{Deserialize, Serialize};
use theme::theme;
use zeroize::Zeroizing;

mod chrome;
mod conversation;
mod fade;
mod field;
mod style;

pub use conversation::{run_conversation, Reply, Update};

use chrome::{backdrop, banner, button, card, header, hint_line, info_block, strength_bar};
use fade::Fade;
use field::{Field, FieldChanged};
use style::apply;

pub const MAX_LIFETIME: Duration = Duration::from_secs(120);

actions!(psst, [Submit, Cancel, FocusNext, FocusPrevious]);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DialogKind {
    Pin,
    Confirm { one_button: bool },
    Message,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogConfig {
    pub kind: DialogKind,
    pub heading: String,
    pub description: Option<String>,
    pub error: Option<String>,
    pub placeholder: String,
    pub ok_label: String,
    pub cancel_label: String,
    pub not_ok_label: Option<String>,
    pub repeat_label: Option<String>,
    pub repeat_error: String,
    pub quality_bar: bool,
    pub choice_label: Option<String>,
    pub choice: bool,
}

pub enum DialogResult {
    Pin {
        secret: Zeroizing<String>,
        choice: bool,
    },
    Confirmed {
        choice: bool,
    },
    Declined,
    Cancelled,
}

struct Icons;

impl AssetSource for Icons {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let svg = match path {
            "lock.svg" => &theme().icons.lock,
            "eye.svg" => &theme().icons.eye,
            "eye-off.svg" => &theme().icons.eye_off,
            _ => return Ok(None),
        };
        Ok(Some(Cow::Owned(svg.clone().into_bytes())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

pub(crate) fn layer_window() -> WindowOptions {
    WindowOptions {
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        kind: WindowKind::LayerShell(LayerShellOptions {
            namespace: "psst".to_string(),
            layer: Layer::Overlay,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            exclusive_zone: Some(px(-1.)),
            keyboard_interactivity: KeyboardInteractivity::Exclusive,
            ..Default::default()
        }),
        ..Default::default()
    }
}

pub(crate) fn arm_watchdog(what: &'static str) {
    std::thread::spawn(move || {
        std::thread::sleep(MAX_LIFETIME);
        eprintln!("psst: {what} timed out; releasing keyboard grab");
        std::process::exit(2);
    });
}

fn cancelable_watchdog(what: &'static str) -> std::sync::mpsc::Sender<()> {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = rx.recv_timeout(MAX_LIFETIME) {
            eprintln!("psst: {what} timed out; releasing keyboard grab");
            std::process::exit(2);
        }
    });
    tx
}

struct Dialog {
    config: DialogConfig,
    pin: Option<gpui::Entity<Field>>,
    repeat: Option<gpui::Entity<Field>>,
    focus: FocusHandle,
    reveal: bool,
    choice: bool,
    mismatch: bool,
    fade: Fade,
    result: Arc<Mutex<DialogResult>>,
}

fn subscribe_field(cx: &mut Context<Dialog>, field: &gpui::Entity<Field>) {
    cx.subscribe(field, |dialog, _, _: &FieldChanged, cx| {
        dialog.mismatch = false;
        cx.notify();
    })
    .detach();
}

impl Dialog {
    fn new(cx: &mut Context<Self>, config: DialogConfig, result: Arc<Mutex<DialogResult>>) -> Self {
        let (pin, repeat) = if let DialogKind::Pin = config.kind {
            let pad = px(20.0 + 16.0);
            let pin = cx.new(|cx| Field::new(cx, true, config.placeholder.clone(), pad));
            subscribe_field(cx, &pin);
            let repeat = config.repeat_label.as_ref().map(|label| {
                let placeholder = if label.is_empty() {
                    "Confirm PIN".to_string()
                } else {
                    label.clone()
                };
                let repeat = cx.new(|cx| Field::new(cx, true, placeholder, px(0.)));
                subscribe_field(cx, &repeat);
                repeat
            });
            (Some(pin), repeat)
        } else {
            (None, None)
        };

        Self {
            choice: config.choice,
            config,
            pin,
            repeat,
            focus: cx.focus_handle(),
            reveal: false,
            mismatch: false,
            fade: Fade::default(),
            result,
        }
    }

    fn initial_focus(&self, cx: &App) -> FocusHandle {
        match &self.pin {
            Some(pin) => pin.read(cx).focus_handle(),
            None => self.focus.clone(),
        }
    }

    fn focus_handles(&self, cx: &App) -> Vec<FocusHandle> {
        let mut handles = Vec::new();
        if let Some(pin) = &self.pin {
            handles.push(pin.read(cx).focus_handle());
        }
        if let Some(repeat) = &self.repeat {
            handles.push(repeat.read(cx).focus_handle());
        }
        handles
    }

    fn step_focus(&mut self, delta: isize, window: &mut gpui::Window, cx: &mut Context<Self>) {
        let handles = self.focus_handles(cx);
        if handles.is_empty() {
            return;
        }
        let current = handles.iter().position(|h| h.is_focused(window));
        let next = match current {
            Some(i) => (i as isize + delta).rem_euclid(handles.len() as isize) as usize,
            None => 0,
        };
        window.focus(&handles[next]);
        cx.notify();
    }

    fn set_reveal(&mut self, reveal: bool, cx: &mut Context<Self>) {
        if self.reveal == reveal {
            return;
        }
        self.reveal = reveal;
        for handle in [&self.pin, &self.repeat].into_iter().flatten() {
            handle.update(cx, |field, _| field.set_masked(!reveal));
        }
        cx.notify();
    }

    fn finish(&self, result: DialogResult, cx: &mut Context<Self>) {
        *self.result.lock().unwrap() = result;
        cx.quit();
    }

    fn submit(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        match self.config.kind {
            DialogKind::Pin => {
                let pin = self.pin.as_ref().unwrap();
                if let Some(repeat) = &self.repeat {
                    let matches = pin.read(cx).value() == repeat.read(cx).value();
                    if !matches {
                        self.mismatch = true;
                        cx.notify();
                        return;
                    }
                }
                let secret = pin.update(cx, |field, _| field.take_value());
                if let Some(repeat) = &self.repeat {
                    repeat.update(cx, |field, _| {
                        field.take_value();
                    });
                }
                let choice = self.choice;
                self.finish(DialogResult::Pin { secret, choice }, cx);
            }
            DialogKind::Confirm { .. } | DialogKind::Message => {
                let choice = self.choice;
                self.finish(DialogResult::Confirmed { choice }, cx);
            }
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        self.finish(DialogResult::Cancelled, cx);
    }

    fn decline(&mut self, cx: &mut Context<Self>) {
        self.finish(DialogResult::Declined, cx);
    }
}

impl Render for Dialog {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let opacity = self.fade.opacity(window);

        let mut body: Vec<AnyElement> = vec![header(&self.config.heading).into_any_element()];
        if let Some(info) = self
            .config
            .description
            .as_deref()
            .and_then(|d| info_block(d.lines()))
        {
            body.push(info);
        }
        if let Some(error) = &self.config.error {
            body.push(banner(error).into_any_element());
        }
        if let DialogKind::Pin = self.config.kind {
            body.push(self.pin_section(cx));
        }
        if let Some(label) = &self.config.choice_label {
            if !matches!(self.config.kind, DialogKind::Message) {
                body.push(self.choice_row(label, cx));
            }
        }
        body.push(self.footer(cx));

        backdrop()
            .track_focus(&self.focus)
            .key_context("Dialog")
            .on_action(cx.listener(|this, _: &Submit, window, cx| this.submit(window, cx)))
            .on_action(cx.listener(|this, _: &Cancel, _, cx| this.cancel(cx)))
            .on_action(
                cx.listener(|this, _: &FocusNext, window, cx| this.step_focus(1, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &FocusPrevious, window, cx| this.step_focus(-1, window, cx)),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.set_reveal(false, cx)),
            )
            .opacity(opacity)
            .child(card(body))
    }
}

impl Dialog {
    fn pin_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.config;
        let pin = self.pin.clone().unwrap();
        let reveal = &theme().reveal.base;

        let eye = div()
            .id("reveal")
            .absolute()
            .top(px(0.))
            .bottom(px(0.))
            .right(px(13.))
            .flex()
            .items_center()
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.set_reveal(true, cx)),
            )
            .child(
                svg()
                    .path(if self.reveal {
                        "eye-off.svg"
                    } else {
                        "eye.svg"
                    })
                    .size(px(reveal.size))
                    .text_color(reveal.text),
            );

        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().relative().w_full().child(pin).child(eye));

        if config.quality_bar {
            section = section.child(strength_bar(
                self.pin.as_ref().unwrap().read(cx).char_count(),
            ));
        }

        if let Some(repeat) = &self.repeat {
            section = section.child(repeat.clone());
            if self.mismatch {
                let msg = if config.repeat_error.is_empty() {
                    "The PINs do not match."
                } else {
                    config.repeat_error.as_str()
                };
                let error = &theme().error.base;
                section = section.child(
                    div()
                        .text_size(px(error.size))
                        .text_color(error.text)
                        .child(msg.to_string()),
                );
            }
        }

        section.into_any_element()
    }

    fn choice_row(&self, label: &str, cx: &mut Context<Self>) -> AnyElement {
        let checkbox = &theme().checkbox;
        let style = if self.choice {
            &checkbox.checked
        } else {
            &checkbox.base
        };
        let mut boxd = apply(
            div().size(px(style.size)).flex().items_center().justify_center(),
            style,
        );
        if self.choice {
            boxd = boxd.child(div().text_size(px(style.size - 4.0)).child("\u{2713}"));
        }

        div()
            .id("choice")
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(style.text)
            .on_click(cx.listener(|this, _, _, cx| {
                this.choice = !this.choice;
                cx.notify();
            }))
            .child(boxd)
            .child(label.to_string())
            .into_any_element()
    }

    fn footer(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = &self.config;
        let dismissible = !matches!(config.kind, DialogKind::Message);

        let mut row = div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .child(hints(config.kind))
            .child(div().flex_1());

        if dismissible {
            if let DialogKind::Confirm { one_button: false } | DialogKind::Pin = config.kind {
                row = row.child(
                    button("cancel", config.cancel_label.clone(), &theme().cancel)
                        .on_click(cx.listener(|this, _, _, cx| this.cancel(cx))),
                );
            }
        }

        if let DialogKind::Confirm { one_button: false } = config.kind {
            if let Some(not_ok) = &config.not_ok_label {
                row = row.child(
                    button("decline", not_ok.clone(), &theme().cancel)
                        .on_click(cx.listener(|this, _, _, cx| this.decline(cx))),
                );
            }
        }

        row = row.child(
            button("confirm", config.ok_label.clone(), &theme().confirm)
                .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
        );

        row.gap(px(10.)).into_any_element()
    }
}

fn hints(kind: DialogKind) -> Div {
    let items: &[(&str, &str)] = match kind {
        DialogKind::Pin => &[("\u{21B5}", "unlock"), ("Esc", "cancel")],
        DialogKind::Confirm { one_button: false } => &[("\u{21B5}", "confirm"), ("esc", "cancel")],
        DialogKind::Confirm { one_button: true } | DialogKind::Message => {
            &[("\u{21B5}", "dismiss")]
        }
    };
    hint_line(items)
}

fn bind_dialog_keys(cx: &mut App) {
    cx.bind_keys([
        gpui::KeyBinding::new("enter", Submit, Some("Dialog")),
        gpui::KeyBinding::new("escape", Cancel, Some("Dialog")),
        gpui::KeyBinding::new("tab", FocusNext, Some("Dialog")),
        gpui::KeyBinding::new("shift-tab", FocusPrevious, Some("Dialog")),
    ]);
}

fn open_dialog_window(cx: &mut App, config: DialogConfig, result: Arc<Mutex<DialogResult>>) {
    cx.open_window(layer_window(), |window, cx| {
        let dialog = cx.new(|cx| Dialog::new(cx, config, result));
        let handle = dialog.read(cx).initial_focus(cx);
        window.focus(&handle);
        dialog
    })
    .unwrap();
}

fn take_result(result: &Arc<Mutex<DialogResult>>) -> DialogResult {
    let mut slot = result.lock().unwrap();
    std::mem::replace(&mut slot, DialogResult::Cancelled)
}

pub fn run_dialog(config: DialogConfig) -> DialogResult {
    let result = Arc::new(Mutex::new(DialogResult::Cancelled));
    let done = cancelable_watchdog("dialog");

    let app_result = result.clone();
    Application::new()
        .with_assets(Icons)
        .run(move |cx: &mut App| {
            bind_dialog_keys(cx);
            open_dialog_window(cx, config.clone(), app_result.clone());
        });

    let _ = done.send(());
    take_result(&result)
}

pub fn run_dialog_stdio() -> DialogResult {
    let result = Arc::new(Mutex::new(DialogResult::Cancelled));

    let app_result = result.clone();
    Application::new()
        .with_assets(Icons)
        .run(move |cx: &mut App| {
            bind_dialog_keys(cx);

            let (tx, rx) = std::sync::mpsc::channel::<Option<DialogConfig>>();
            std::thread::spawn(move || {
                let mut line = String::new();
                let config = match std::io::stdin().read_line(&mut line) {
                    Ok(n) if n > 0 => serde_json::from_str::<DialogConfig>(line.trim()).ok(),
                    _ => None,
                };
                let _ = tx.send(config);
            });

            let app_result = app_result.clone();
            cx.spawn(async move |cx| loop {
                match rx.try_recv() {
                    Ok(Some(config)) => {
                        let _ = cx.update(|cx| {
                            arm_watchdog("dialog");
                            open_dialog_window(cx, config, app_result.clone());
                        });
                        break;
                    }
                    Ok(None) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = cx.update(|cx| cx.quit());
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        cx.background_executor()
                            .timer(Duration::from_millis(20))
                            .await;
                    }
                }
            })
            .detach();
        });

    take_result(&result)
}
