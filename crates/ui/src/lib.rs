use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use iced::widget::canvas::{Action, Geometry, Program};
use iced::widget::Column;
use iced::widget::{
    button, canvas as canvas_widget, checkbox, column, container, mouse_area, progress_bar, row,
    stack, svg, text, text_input, Space,
};
use iced::{Background, Border, Color, Element, Length, Shadow, Task, Theme, Vector};
use iced_layershell::build_pattern::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::to_layer_message;

use theme::theme;

const FADE_IN: f32 = 0.18;

pub const MAX_LIFETIME: Duration = Duration::from_secs(120);

static PIN_ID: LazyLock<iced::widget::Id> = LazyLock::new(|| iced::widget::Id::new("pin"));

fn icon_handle(slot: &'static OnceLock<svg::Handle>, markup: &str) -> svg::Handle {
    slot.get_or_init(|| svg::Handle::from_memory(markup.to_string().into_bytes()))
        .clone()
}

pub(crate) fn lock_icon() -> svg::Handle {
    static SLOT: OnceLock<svg::Handle> = OnceLock::new();
    icon_handle(&SLOT, &theme().title_icon.svg)
}

fn eye_icon(reveal: bool) -> svg::Handle {
    static EYE: OnceLock<svg::Handle> = OnceLock::new();
    static EYE_OFF: OnceLock<svg::Handle> = OnceLock::new();
    if reveal {
        icon_handle(&EYE_OFF, &theme().reveal.eye_off)
    } else {
        icon_handle(&EYE, &theme().reveal.eye)
    }
}

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
            eprintln!("psst: dialog timed out; releasing keyboard grab");
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

pub(crate) fn namespace() -> String {
    String::from("psst")
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
    let window = &theme().window;
    let spacing = window.spacing;
    let mut card = column![header(config, state.opacity < 1.0)].width(Length::Fill);

    if let Some(info) = info_block(config) {
        card = gap(card, spacing).push(info);
    }
    if let Some(error) = &config.error {
        card = gap(card, spacing).push(banner(error));
    }

    if let DialogKind::Pin = config.kind {
        card = gap(card, spacing).push(pin_section(state));
    }

    if let Some(label) = &config.choice_label {
        if !matches!(config.kind, DialogKind::Message) {
            card = gap(card, spacing).push(choice_row(label, state.choice));
        }
    }

    card = gap(card, spacing).push(footer(state));

    let card = container(card)
        .padding(edges(window.padding))
        .max_width(window.width)
        .width(Length::Fill)
        .style(card_style);

    container(card).center(Length::Fill).padding(64).into()
}

pub(crate) fn edges(padding: theme::Padding) -> iced::Padding {
    iced::Padding {
        top: padding.y,
        bottom: padding.y,
        left: padding.x,
        right: padding.x,
    }
}

fn gap(card: Column<'_, Message>, height: f32) -> Column<'_, Message> {
    card.push(Space::new().height(Length::Fixed(height)))
}

fn choice_row(label: &str, checked: bool) -> Element<'_, Message> {
    let base = &theme().checkbox;
    checkbox(checked)
        .label(label.to_string())
        .on_toggle(Message::ToggleChoice)
        .size(base.box_size)
        .text_size(base.font.size)
        .spacing(base.spacing)
        .style(|_theme, status| {
            let is_checked = matches!(
                status,
                checkbox::Status::Active { is_checked: true }
                    | checkbox::Status::Hovered { is_checked: true }
                    | checkbox::Status::Disabled { is_checked: true }
            );
            let t = if is_checked {
                &theme().checkbox_checked
            } else {
                &theme().checkbox.paint
            };
            checkbox::Style {
                background: Background::Color(t.background),
                icon_color: t.check,
                border: Border {
                    color: t.border.color,
                    width: t.border.size,
                    radius: t.radius.into(),
                },
                text_color: Some(t.color),
            }
        })
        .into()
}

fn header(config: &DialogConfig, animating: bool) -> Element<'_, Message> {
    let ti = &theme().title_icon;
    let tile = ti.size + ti.padding * 2.0;
    let ticker = canvas_widget(Ticker { animating })
        .width(Length::Fixed(tile))
        .height(Length::Fixed(tile));
    let glyph = container(
        svg(lock_icon())
            .width(Length::Fixed(ti.size))
            .height(Length::Fixed(ti.size))
            .style(|_theme, _status| svg::Style {
                color: Some(theme().title_icon.color),
            }),
    )
    .center(Length::Fixed(tile));
    let icon = container(stack![ticker, glyph]).style(icon_style);

    row![
        icon,
        text(config.heading.clone())
            .size(theme().title.font.size)
            .font(theme().title.font.family)
            .color(theme().title.color)
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
    let label = &theme().description.label;
    let value = &theme().description.value;
    if let Some((name, text_value)) = line.split_once(": ") {
        if !name.is_empty() && name.len() <= 24 && !text_value.trim().is_empty() {
            return row![
                text(format!("{name}:"))
                    .size(label.font.size)
                    .font(label.font.family)
                    .color(label.color),
                text(text_value.trim().to_string())
                    .size(value.font.size)
                    .font(value.font.family)
                    .color(value.color)
                    .width(Length::Fill),
            ]
            .spacing(8)
            .into();
        }
    }
    text(line.to_string())
        .size(value.font.size)
        .font(value.font.family)
        .color(value.color)
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
                .girth(Length::Fixed(theme().strength.height))
                .style(move |_theme| {
                    let t = &theme().strength;
                    progress_bar::Style {
                        background: Background::Color(t.track),
                        bar: Background::Color(fill),
                        border: Border {
                            color: t.border.color,
                            width: t.border.size,
                            radius: t.radius.into(),
                        },
                    }
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
            section = section.push(
                text(msg.to_string())
                    .size(theme().error.font.size)
                    .color(theme().error.color),
            );
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
    let field = &theme().field;
    let padding = iced::Padding {
        top: field.padding.y,
        bottom: field.padding.y,
        left: field.padding.x,
        right: if with_toggle {
            field.padding.x + theme().reveal.size + 16.0
        } else {
            field.padding.x
        },
    };
    let mut input = text_input(placeholder, value)
        .secure(!reveal)
        .on_input(on_input)
        .font(field.font.family)
        .size(field.font.size)
        .padding(padding)
        .width(Length::Fill)
        .style(field_input);
    if let Some(id) = id {
        input = input.id(id);
    }

    if !with_toggle {
        return input.into();
    }

    let eye = mouse_area(
        svg(eye_icon(reveal))
            .width(Length::Fixed(theme().reveal.size))
            .height(Length::Fixed(theme().reveal.size))
            .style(|_theme, _status| svg::Style {
                color: Some(theme().reveal.color),
            }),
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
                button(text(config.cancel_label.clone()).size(theme().cancel.font.size))
                    .on_press(Message::Cancel)
                    .padding(edges(theme().cancel.padding))
                    .style(text_button),
            );
        }
    }

    if let DialogKind::Confirm { one_button: false } = config.kind {
        if let Some(not_ok) = &config.not_ok_label {
            row = row.push(
                button(text(not_ok.clone()).size(theme().cancel.font.size))
                    .on_press(Message::Decline)
                    .padding(edges(theme().cancel.padding))
                    .style(text_button),
            );
        }
    }

    row.push(
        button(text(config.ok_label.clone()).size(theme().confirm.font.size))
            .on_press(Message::Confirm)
            .padding(edges(theme().confirm.padding))
            .style(pill_button),
    )
    .into()
}

fn hints(config: &DialogConfig) -> Element<'_, Message> {
    let items: &[(&str, &str)] = match config.kind {
        DialogKind::Pin => &[("\u{21B5}", "unlock"), ("Esc", "cancel")],
        DialogKind::Confirm { one_button: false } => &[("\u{21B5}", "confirm"), ("esc", "cancel")],
        DialogKind::Confirm { one_button: true } | DialogKind::Message => {
            &[("\u{21B5}", "dismiss")]
        }
    };

    let key = &theme().hint.key;
    let word = &theme().hint.word;
    let mut line = row![]
        .spacing(theme().hint.spacing)
        .align_y(iced::Alignment::Center);
    for (i, (glyph, label)) in items.iter().enumerate() {
        if i > 0 {
            line = line.push(text("\u{00B7}").size(word.font.size).color(word.color));
        }
        line = line.push(
            text(*glyph)
                .font(key.font.family)
                .size(key.font.size)
                .color(key.color),
        );
        line = line.push(
            text(*label)
                .font(word.font.family)
                .size(word.font.size)
                .color(word.color),
        );
    }
    line.into()
}

fn banner<'a>(message: &str) -> Element<'a, Message> {
    container(
        text(message.to_string())
            .size(theme().error.font.size)
            .color(theme().error.color),
    )
    .padding(edges(theme().error.padding))
    .width(Length::Fill)
    .style(|_theme| {
        let t = &theme().error;
        container::Style {
            background: Some(Background::Color(t.background)),
            border: Border {
                color: t.border.color,
                width: t.border.size,
                radius: t.radius.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

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
    let backdrop = theme().backdrop.color;
    iced::theme::Style {
        background_color: Color {
            a: backdrop.a * state.opacity,
            ..backdrop
        },
        text_color: theme().title.color,
    }
}

pub(crate) fn card_style(_theme: &Theme) -> container::Style {
    let t = &theme().window;
    container::Style {
        text_color: Some(theme().title.color),
        background: Some(Background::Color(t.background)),
        border: Border {
            color: t.border.color,
            width: t.border.size,
            radius: t.radius.into(),
        },
        shadow: Shadow {
            color: t.shadow.color,
            offset: Vector::new(0.0, t.shadow.offset),
            blur_radius: t.shadow.blur,
        },
        ..Default::default()
    }
}

pub(crate) fn icon_style(_theme: &Theme) -> container::Style {
    let t = &theme().title_icon;
    container::Style {
        background: Some(Background::Color(t.background)),
        border: Border {
            color: t.border.color,
            width: t.border.size,
            radius: t.radius.into(),
        },
        ..Default::default()
    }
}

pub(crate) fn field_input(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let t = match status {
        text_input::Status::Focused { .. } => &theme().field_focus,
        _ => &theme().field.paint,
    };
    text_input::Style {
        background: Background::Color(t.background),
        border: Border {
            color: t.border.color,
            width: t.border.size,
            radius: t.radius.into(),
        },
        icon: Color::TRANSPARENT,
        placeholder: t.placeholder,
        value: t.color,
        selection: t.selection,
    }
}

pub(crate) fn pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let t = match status {
        button::Status::Hovered | button::Status::Pressed => &theme().confirm_hover,
        _ => &theme().confirm.paint,
    };
    button_style(t)
}

pub(crate) fn text_button(_theme: &Theme, status: button::Status) -> button::Style {
    let t = match status {
        button::Status::Hovered | button::Status::Pressed => &theme().cancel_hover,
        _ => &theme().cancel.paint,
    };
    button_style(t)
}

fn button_style(t: &theme::ButtonPaint) -> button::Style {
    button::Style {
        background: Some(Background::Color(t.background)),
        text_color: t.color,
        border: Border {
            color: t.border.color,
            width: t.border.size,
            radius: t.radius.into(),
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
    let t = &theme().strength;
    if value < 0.5 {
        lerp_color(t.weak, t.medium, value / 0.5)
    } else {
        lerp_color(t.medium, t.strong, (value - 0.5) / 0.5)
    }
}
