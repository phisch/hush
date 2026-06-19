use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use iced::widget::canvas::{Action, Geometry, Program};
use iced::widget::Column;
use iced::widget::{
    button, canvas as canvas_widget, checkbox, column, container, mouse_area, progress_bar, row,
    stack, svg, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Font, Length, Shadow, Task, Theme, Vector};
use iced_layershell::build_pattern::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::to_layer_message;

const fn hex(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

const BACKDROP: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.8);
const CARD_BG: Color = Color::from_rgb(0.07, 0.07, 0.07);
const CARD_BORDER: Color = Color::from_rgb(0.13, 0.13, 0.13);
const FIELD_BG: Color = hex(0x0F, 0x0F, 0x0F);
const FIELD_BG_FOCUS: Color = hex(0x1D, 0x1D, 0x1D);
const FIELD_BORDER: Color = hex(0x27, 0x27, 0x27);
const TITLE: Color = Color::from_rgb(0.94, 0.94, 0.95);
const DESC: Color = Color::from_rgb(0.62, 0.63, 0.67);
const LABEL: Color = Color::from_rgb(0.45, 0.46, 0.50);
const PLACEHOLDER: Color = Color::from_rgb(0.42, 0.43, 0.47);
const SHOW: Color = Color::from_rgb(0.60, 0.61, 0.66);
const STRENGTH_TRACK: Color = hex(0x22, 0x22, 0x22);
const STRENGTH_WEAK: Color = hex(0xE5, 0x48, 0x4D);
const STRENGTH_MEDIUM: Color = hex(0xE6, 0xB3, 0x40);
const STRENGTH_STRONG: Color = hex(0x5C, 0xC8, 0x6B);
const CANCEL_FG: Color = hex(0x8F, 0x8F, 0x8F);
const CANCEL_HOVER_BG: Color = hex(0x1F, 0x1F, 0x1F);
const PILL_FG: Color = Color::from_rgb(0.84, 0.89, 1.0);
const HINT_DIM: Color = hex(0x39, 0x39, 0x39);
const HINT_KEY: Color = hex(0xA5, 0xA5, 0xA5);
const LOCK: Color = Color::from_rgb(0.74, 0.81, 0.98);
const ICON_BG: Color = Color::from_rgba(0.35, 0.0, 1.0, 0.4);
const ACCENT: Color = Color::from_rgba(0.35, 0.0, 0.1, 0.4);
const DANGER: Color = Color::from_rgb(0.92, 0.46, 0.47);

const MONO: Font = Font::MONOSPACE;
const FADE_IN: f32 = 0.18;

/// Hard cap on how long the dialog may hold the exclusive keyboard grab.
///
/// A supervising parent (e.g. the keyring daemon) should set its own kill
/// timeout *longer* than this, so the dialog always self-terminates first.
pub const MAX_LIFETIME: Duration = Duration::from_secs(120);

static PIN_ID: LazyLock<iced::widget::Id> = LazyLock::new(|| iced::widget::Id::new("pin"));

const LOCK_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" height="40px" viewBox="0 -960 960 960" width="40px" fill="#e3e3e3"><path d="M226.67-80q-27.5 0-47.09-19.58Q160-119.17 160-146.67v-422.66q0-27.5 19.58-47.09Q199.17-636 226.67-636h60v-90.67q0-80.23 56.57-136.78T480.07-920q80.26 0 136.76 56.55 56.5 56.55 56.5 136.78V-636h60q27.5 0 47.09 19.58Q800-596.83 800-569.33v422.66q0 27.5-19.58 47.09Q760.83-80 733.33-80H226.67Zm0-66.67h506.66v-422.66H226.67v422.66Zm308.5-155.85Q558-325.04 558-356.67q0-31-22.95-55.16Q512.11-436 479.89-436t-55.06 24.17Q402-387.67 402-356.33q0 31.33 22.95 53.83 22.94 22.5 55.16 22.5t55.06-22.52ZM353.33-636h253.34v-90.67q0-52.77-36.92-89.72-36.93-36.94-89.67-36.94-52.75 0-89.75 36.94-37 36.95-37 89.72V-636ZM226.67-146.67v-422.66 422.66Z"/></svg>"##;

static LOCK_ICON: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(LOCK_SVG));

const EYE_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="#e3e3e3"><path d="M599-361q49-49 49-119t-49-119q-49-49-119-49t-119 49q-49 49-49 119t49 119q49 49 119 49t119-49Zm-187-51q-28-28-28-68t28-68q28-28 68-28t68 28q28 28 28 68t-28 68q-28 28-68 28t-68-28ZM220-270.5Q103-349 48-480q55-131 172-209.5T480-768q143 0 260 78.5T912-480q-55 131-172 209.5T480-192q-143 0-260-78.5ZM480-480Zm207 158q95-58 146-158-51-100-146-158t-207-58q-112 0-207 58T127-480q51 100 146 158t207 58q112 0 207-58Z"/></svg>"##;

const EYE_OFF_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="#e3e3e3"><path d="m637-425-62-62q4-38-23-65.5T487-576l-62-62q13-5 27-7.5t28-2.5q70 0 119 49t49 119q0 14-2.5 28t-8.5 27Zm133 133-52-52q36-28 65.5-61.5T833-480q-49-101-144.5-158.5T480-696q-26 0-51 3t-49 10l-58-58q38-15 77.5-21t80.5-6q143 0 261.5 77.5T912-480q-22 57-58.5 103.5T770-292Zm-2 202L638-220q-38 14-77.5 21t-80.5 7q-143 0-261.5-77.5T48-480q22-57 58-104t84-85L90-769l51-51 678 679-51 51ZM241-617q-35 28-65 61.5T127-480q49 101 144.5 158.5T480-264q26 0 51-3.5t50-9.5l-45-45q-14 5-28 7.5t-28 2.5q-70 0-119-49t-49-119q0-14 3.5-28t6.5-28l-81-81Zm287 89Zm-96 96Z"/></svg>"##;

static EYE_ICON: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(EYE_SVG));
static EYE_OFF_ICON: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(EYE_OFF_SVG));

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
    /// When set, a checkbox with this label is shown (e.g. "Automatically
    /// unlock this keyring whenever I'm logged in").
    pub choice_label: Option<String>,
    /// The checkbox's initial state.
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

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    PinChanged(String),
    RepeatChanged(String),
    Reveal(bool),
    ToggleChoice(bool),
    Confirm,
    Decline,
    Cancel,
    FocusNext,
    FocusPrevious,
    Tick,
}

struct State {
    config: DialogConfig,
    pin: Zeroizing<String>,
    repeat: Zeroizing<String>,
    reveal: bool,
    choice: bool,
    mismatch: bool,
    done: bool,
    started: Option<Instant>,
    opacity: f32,
    result: Arc<Mutex<DialogResult>>,
}

impl State {
    fn finish(&mut self, result: DialogResult) -> Task<Message> {
        self.done = true;
        *self.result.lock().unwrap() = result;
        iced::exit()
    }
}

pub fn run_dialog(config: DialogConfig) -> DialogResult {
    let result = Arc::new(Mutex::new(DialogResult::Cancelled));

    let boot_result = result.clone();
    let boot = move || {
        let state = State {
            config: config.clone(),
            pin: Zeroizing::new(String::new()),
            repeat: Zeroizing::new(String::new()),
            reveal: false,
            choice: config.choice,
            mismatch: false,
            done: false,
            started: None,
            opacity: 0.0,
            result: boot_result.clone(),
        };
        (state, iced::widget::operation::focus(PIN_ID.clone()))
    };

    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        if let Err(std::sync::mpsc::RecvTimeoutError::Timeout) = done_rx.recv_timeout(MAX_LIFETIME)
        {
            eprintln!("hush: dialog timed out; releasing keyboard grab");
            std::process::exit(2);
        }
    });

    let _ = application(boot, namespace, update, view)
        .style(style)
        .subscription(subscription)
        .settings(Settings {
            layer_settings: LayerShellSettings {
                layer: Layer::Overlay,
                anchor: Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right,
                exclusive_zone: -1,
                keyboard_interactivity: KeyboardInteractivity::Exclusive,
                size: None,
                ..Default::default()
            },
            ..Default::default()
        })
        .run();

    let _ = done_tx.send(());

    let mut slot = result.lock().unwrap();
    std::mem::replace(&mut slot, DialogResult::Cancelled)
}

fn namespace() -> String {
    String::from("pinentry")
}

fn subscription(_state: &State) -> iced::Subscription<Message> {
    iced::event::listen_with(handle_event)
}

fn handle_event(
    event: iced::Event,
    _status: iced::event::Status,
    _id: iced::window::Id,
) -> Option<Message> {
    use iced::keyboard::{key::Named, Event::KeyPressed, Key};

    match event {
        iced::Event::Keyboard(KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        }) => Some(Message::Cancel),
        iced::Event::Keyboard(KeyPressed {
            key: Key::Named(Named::Enter),
            ..
        }) => Some(Message::Confirm),
        iced::Event::Keyboard(KeyPressed {
            key: Key::Named(Named::Tab),
            modifiers,
            ..
        }) => Some(if modifiers.shift() {
            Message::FocusPrevious
        } else {
            Message::FocusNext
        }),
        _ => None,
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    if state.done {
        return Task::none();
    }
    match message {
        Message::PinChanged(value) => {
            state.pin = Zeroizing::new(value);
            state.mismatch = false;
            Task::none()
        }
        Message::RepeatChanged(value) => {
            state.repeat = Zeroizing::new(value);
            state.mismatch = false;
            Task::none()
        }
        Message::Reveal(reveal) => {
            state.reveal = reveal;
            Task::none()
        }
        Message::Confirm => match state.config.kind {
            DialogKind::Pin => {
                if state.config.repeat_label.is_some() && *state.pin != *state.repeat {
                    state.mismatch = true;
                    Task::none()
                } else {
                    let pin = std::mem::replace(&mut state.pin, Zeroizing::new(String::new()));
                    let choice = state.choice;
                    state.finish(DialogResult::Pin {
                        secret: pin,
                        choice,
                    })
                }
            }
            DialogKind::Confirm { .. } | DialogKind::Message => {
                let choice = state.choice;
                state.finish(DialogResult::Confirmed { choice })
            }
        },
        Message::ToggleChoice(value) => {
            state.choice = value;
            Task::none()
        }
        Message::Decline => state.finish(DialogResult::Declined),
        Message::Cancel => state.finish(DialogResult::Cancelled),
        Message::FocusNext => iced::widget::operation::focus_next(),
        Message::FocusPrevious => iced::widget::operation::focus_previous(),
        Message::Tick => {
            let started = *state.started.get_or_insert_with(Instant::now);
            let t = (started.elapsed().as_secs_f32() / FADE_IN).clamp(0.0, 1.0);
            state.opacity = 1.0 - (1.0 - t) * (1.0 - t);
            Task::none()
        }
        _ => Task::none(),
    }
}

fn view(state: &State) -> Element<'_, Message> {
    let config = &state.config;
    let mut card = column![header(config, state.opacity < 1.0)].width(Length::Fill);

    if let Some(info) = info_block(config) {
        card = gap(card, 16.0).push(info);
    }
    if let Some(error) = &config.error {
        card = gap(card, 14.0).push(banner(error));
    }

    if let DialogKind::Pin = config.kind {
        card = gap(card, 20.0).push(pin_section(state));
    }

    if let Some(label) = &config.choice_label {
        if !matches!(config.kind, DialogKind::Message) {
            card = gap(card, 16.0).push(choice_row(label, state.choice));
        }
    }

    card = gap(card, 22.0).push(footer(state));

    let card = container(card)
        .padding([26, 28])
        .max_width(430)
        .width(Length::Fill)
        .style(card_style);

    container(card).center(Length::Fill).padding(64).into()
}

fn gap(card: Column<'_, Message>, height: f32) -> Column<'_, Message> {
    card.push(Space::new().height(Length::Fixed(height)))
}

fn choice_row(label: &str, checked: bool) -> Element<'_, Message> {
    checkbox(checked)
        .label(label.to_string())
        .on_toggle(Message::ToggleChoice)
        .size(16.0)
        .text_size(13.0)
        .spacing(10.0)
        .style(|_theme, status| {
            let is_checked = matches!(
                status,
                checkbox::Status::Active { is_checked: true }
                    | checkbox::Status::Hovered { is_checked: true }
                    | checkbox::Status::Disabled { is_checked: true }
            );
            checkbox::Style {
                background: Background::Color(FIELD_BG),
                icon_color: Color::from_rgb(1.0, 1.0, 1.0),
                border: Border {
                    color: FIELD_BORDER,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: Some(if is_checked {
                    DESC
                } else {
                    DESC.scale_alpha(0.4)
                }),
            }
        })
        .into()
}

fn header(config: &DialogConfig, animating: bool) -> Element<'_, Message> {
    let ticker = canvas_widget(Ticker { animating })
        .width(Length::Fixed(42.0))
        .height(Length::Fixed(42.0));
    let glyph = container(
        svg(LOCK_ICON.clone())
            .width(Length::Fixed(22.0))
            .height(Length::Fixed(22.0))
            .style(|_theme, _status| svg::Style { color: Some(LOCK) }),
    )
    .center(Length::Fixed(42.0));
    let icon = container(stack![ticker, glyph]).style(icon_style);

    row![
        icon,
        text(config.heading.clone())
            .size(24)
            .font(Font::with_name("Inter"))
            .color(TITLE)
    ]
    .spacing(14)
    .align_y(iced::Alignment::Center)
    .into()
}

fn info_block(config: &DialogConfig) -> Option<Element<'_, Message>> {
    let description = config.description.as_ref()?;

    let mut block = column![].spacing(6);
    let mut any = false;
    for line in description.lines().filter(|l| !l.trim().is_empty()) {
        any = true;
        block = block.push(info_line(line.trim()));
    }

    any.then(|| block.into())
}

fn info_line(line: &str) -> Element<'_, Message> {
    if let Some((label, value)) = line.split_once(": ") {
        if !label.is_empty() && label.len() <= 24 && !value.trim().is_empty() {
            return row![
                text(format!("{label}:")).size(13).color(LABEL),
                text(value.trim().to_string())
                    .size(13)
                    .font(MONO)
                    .color(DESC)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .into();
        }
    }
    text(line.to_string())
        .size(13)
        .color(DESC)
        .width(Length::Fill)
        .into()
}

fn pin_section(state: &State) -> Element<'_, Message> {
    let config = &state.config;
    let mut section = column![pin_field(
        Some(PIN_ID.clone()),
        &config.placeholder,
        &state.pin,
        state.reveal,
        Message::PinChanged,
        true,
    )]
    .spacing(12);

    if config.quality_bar {
        let value = strength(&state.pin);
        let fill = strength_color(value);
        section = section.push(
            progress_bar(0.0..=1.0, value)
                .girth(Length::Fixed(6.0))
                .style(move |_theme| progress_bar::Style {
                    background: Background::Color(STRENGTH_TRACK),
                    bar: Background::Color(fill),
                    border: Border {
                        radius: 3.0.into(),
                        ..Default::default()
                    },
                }),
        );
    }

    if let Some(repeat_label) = &config.repeat_label {
        let placeholder = if repeat_label.is_empty() {
            "Confirm PIN"
        } else {
            repeat_label.as_str()
        };
        section = section.push(pin_field(
            None,
            placeholder,
            &state.repeat,
            state.reveal,
            Message::RepeatChanged,
            false,
        ));
        if state.mismatch {
            let msg = if config.repeat_error.is_empty() {
                "The PINs do not match."
            } else {
                config.repeat_error.as_str()
            };
            section = section.push(text(msg.to_string()).size(13).color(DANGER));
        }
    }

    section.into()
}

fn pin_field<'a>(
    id: Option<iced::widget::Id>,
    placeholder: &str,
    value: &str,
    reveal: bool,
    on_input: impl Fn(String) -> Message + 'a,
    with_toggle: bool,
) -> Element<'a, Message> {
    // The text_input is the field box (background, border, padding) so its
    // background can be focus-aware; the Show button floats over its right edge.
    let padding = iced::Padding {
        top: 11.0,
        right: if with_toggle { 64.0 } else { 13.0 },
        bottom: 11.0,
        left: 13.0,
    };
    let mut input = text_input(placeholder, value)
        .secure(!reveal)
        .on_input(on_input)
        .font(MONO)
        .size(15)
        .padding(padding)
        .width(Length::Fill)
        .style(field_input);
    if let Some(id) = id {
        input = input.id(id);
    }

    if !with_toggle {
        return input.into();
    }

    let icon = if reveal {
        EYE_OFF_ICON.clone()
    } else {
        EYE_ICON.clone()
    };
    let eye = mouse_area(
        svg(icon)
            .width(Length::Fixed(20.0))
            .height(Length::Fixed(20.0))
            .style(|_theme, _status| svg::Style { color: Some(SHOW) }),
    )
    .on_press(Message::Reveal(true))
    .on_release(Message::Reveal(false))
    .on_exit(Message::Reveal(false));
    let reveal_control = container(eye)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Center)
        .padding(iced::Padding {
            top: 0.0,
            right: 13.0,
            bottom: 0.0,
            left: 0.0,
        });

    stack![input, reveal_control].into()
}

fn footer(state: &State) -> Element<'_, Message> {
    let config = &state.config;
    let mut row = row![hints(config)]
        .spacing(10)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill);
    row = row.push(Space::new().width(Length::Fill));

    let dismissible = !matches!(config.kind, DialogKind::Message);
    if dismissible {
        if let DialogKind::Confirm { one_button: false } | DialogKind::Pin = config.kind {
            row = row.push(
                button(text(config.cancel_label.clone()).size(13))
                    .on_press(Message::Cancel)
                    .padding([9, 12])
                    .style(text_button),
            );
        }
    }

    if let DialogKind::Confirm { one_button: false } = config.kind {
        if let Some(not_ok) = &config.not_ok_label {
            row = row.push(
                button(text(not_ok.clone()).size(13))
                    .on_press(Message::Decline)
                    .padding([9, 12])
                    .style(text_button),
            );
        }
    }

    row.push(
        button(text(config.ok_label.clone()).size(13))
            .on_press(Message::Confirm)
            .padding([9, 18])
            .style(pill_button),
    )
    .into()
}

fn hints(config: &DialogConfig) -> Element<'_, Message> {
    let items: &[(&str, &str)] = match config.kind {
        DialogKind::Pin => &[("\u{21B5}", "unlock"), ("esc", "cancel")],
        DialogKind::Confirm { one_button: false } => &[("\u{21B5}", "confirm"), ("esc", "cancel")],
        DialogKind::Confirm { one_button: true } | DialogKind::Message => {
            &[("\u{21B5}", "dismiss")]
        }
    };

    let mut line = row![].spacing(6).align_y(iced::Alignment::Center);
    for (i, (key, word)) in items.iter().enumerate() {
        if i > 0 {
            line = line.push(text("\u{00B7}").size(12).color(HINT_DIM));
        }
        line = line.push(text(*key).font(MONO).size(12).color(HINT_KEY));
        line = line.push(text(*word).size(12).color(HINT_DIM));
    }
    line.into()
}

fn banner<'a>(message: &str) -> Element<'a, Message> {
    container(text(message.to_string()).size(13).color(DANGER))
        .padding([8, 12])
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(Color { a: 0.10, ..DANGER })),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// An invisible canvas whose only job is to republish [`Message::Tick`] on
/// every redraw while animating — that self-sustaining loop is what drives the
/// fade-in under `iced_layershell` (a plain timer subscription doesn't keep
/// firing). It draws nothing; the lock glyph is an SVG layered on top.
struct Ticker {
    animating: bool,
}

impl Program<Message> for Ticker {
    type State = ();

    fn update(
        &self,
        _state: &mut (),
        event: &iced::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Option<Action<Message>> {
        let is_frame = matches!(
            event,
            iced::Event::Window(iced::window::Event::RedrawRequested(_))
        );
        (self.animating && is_frame).then(|| Action::publish(Message::Tick))
    }

    fn draw(
        &self,
        _state: &(),
        _renderer: &iced::Renderer,
        _theme: &Theme,
        _bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        Vec::new()
    }
}

fn style(state: &State, _theme: &Theme) -> iced::theme::Style {
    iced::theme::Style {
        background_color: Color {
            a: BACKDROP.a * state.opacity,
            ..BACKDROP
        },
        text_color: TITLE,
    }
}

fn card_style(_theme: &Theme) -> container::Style {
    container::Style {
        text_color: Some(TITLE),
        background: Some(Background::Color(CARD_BG)),
        border: Border {
            color: CARD_BORDER,
            width: 2.0,
            radius: 16.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.6),
            offset: Vector::new(0.0, 18.0),
            blur_radius: 56.0,
        },
        ..Default::default()
    }
}

fn icon_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(ICON_BG)),
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn field_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let background = match status {
        text_input::Status::Focused { .. } => FIELD_BG_FOCUS,
        _ => FIELD_BG,
    };
    text_input::Style {
        background: Background::Color(background),
        border: Border {
            color: FIELD_BORDER,
            width: 1.0,
            radius: 9.0.into(),
        },
        icon: Color::TRANSPARENT,
        placeholder: PLACEHOLDER,
        value: TITLE,
        selection: Color { a: 0.35, ..ACCENT },
    }
}

fn pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => Color::from_rgba(0.35, 0.0, 1.0, 0.6),
        _ => Color::from_rgba(0.35, 0.0, 1.0, 0.4),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: PILL_FG,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn text_button(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Some(Background::Color(CANCEL_HOVER_BG))
        }
        _ => None,
    };
    button::Style {
        background,
        text_color: CANCEL_FG,
        border: Border {
            radius: 8.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn strength(pin: &str) -> f32 {
    (pin.chars().count() as f32 / 20.0).clamp(0.0, 1.0)
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
    )
}

fn strength_color(value: f32) -> Color {
    if value < 0.5 {
        lerp_color(STRENGTH_WEAK, STRENGTH_MEDIUM, value / 0.5)
    } else {
        lerp_color(STRENGTH_MEDIUM, STRENGTH_STRONG, (value - 0.5) / 0.5)
    }
}
