use std::f32::consts::{PI, TAU};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

use iced::widget::canvas::path::Arc as PathArc;
use iced::widget::canvas::{Action, Frame, Geometry, Path, Program, Stroke};
use iced::widget::Column;
use iced::widget::{
    button, canvas as canvas_widget, column, container, progress_bar, row, text, text_input, Space,
};
use iced::{
    Background, Border, Color, Element, Font, Length, Point, Radians, Shadow, Size, Task, Theme,
    Vector,
};
use iced_layershell::build_pattern::application;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::to_layer_message;

use pinentry::{ConfirmOutcome, Frontend, PinOutcome, Settings as ProtoSettings};

const BACKDROP: Color = Color::from_rgba(0.0, 0.0, 0.0, 0.8);
const CARD_BG: Color = Color::from_rgb(0.125, 0.125, 0.133);
const CARD_BORDER: Color = Color::from_rgb(0.21, 0.21, 0.223);
const FIELD_BG: Color = Color::from_rgb(0.082, 0.082, 0.090);
const FIELD_BORDER: Color = Color::from_rgb(0.21, 0.21, 0.223);
const FIELD_BORDER_FOCUS: Color = Color::from_rgba(0.36, 0.56, 0.96, 0.55);
const TITLE: Color = Color::from_rgb(0.94, 0.94, 0.95);
const DESC: Color = Color::from_rgb(0.62, 0.63, 0.67);
const LABEL: Color = Color::from_rgb(0.45, 0.46, 0.50);
const PLACEHOLDER: Color = Color::from_rgb(0.42, 0.43, 0.47);
const SHOW: Color = Color::from_rgb(0.60, 0.61, 0.66);
const CANCEL_FG: Color = Color::from_rgb(0.76, 0.77, 0.80);
const PILL_FG: Color = Color::from_rgb(0.84, 0.89, 1.0);
const HINT_DIM: Color = Color::from_rgb(0.40, 0.41, 0.45);
const HINT_KEY: Color = Color::from_rgb(0.58, 0.59, 0.64);
const LOCK: Color = Color::from_rgb(0.74, 0.81, 0.98);
const ICON_BG: Color = Color::from_rgba(0.36, 0.56, 0.96, 0.10);
const ACCENT: Color = Color::from_rgb(0.36, 0.56, 0.96);
const DANGER: Color = Color::from_rgb(0.92, 0.46, 0.47);

const MONO: Font = Font::MONOSPACE;
const FADE_IN: f32 = 0.18;

static PIN_ID: LazyLock<iced::widget::Id> = LazyLock::new(|| iced::widget::Id::new("pin"));

#[derive(Debug, Clone, Copy)]
pub enum DialogKind {
    Pin,
    Confirm { one_button: bool },
    Message,
}

#[derive(Debug, Clone)]
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
}

enum DialogResult {
    Pin(String),
    Confirmed,
    Declined,
    Cancelled,
}

#[to_layer_message]
#[derive(Debug, Clone)]
enum Message {
    PinChanged(String),
    RepeatChanged(String),
    ToggleReveal,
    Confirm,
    Decline,
    Cancel,
    Tick,
}

struct State {
    config: DialogConfig,
    pin: String,
    repeat: String,
    reveal: bool,
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

fn run_dialog(config: DialogConfig) -> DialogResult {
    let result = Arc::new(Mutex::new(DialogResult::Cancelled));

    let boot_result = result.clone();
    let boot = move || {
        let state = State {
            config: config.clone(),
            pin: String::new(),
            repeat: String::new(),
            reveal: false,
            mismatch: false,
            done: false,
            started: None,
            opacity: 0.0,
            result: boot_result.clone(),
        };
        (state, iced::widget::operation::focus(PIN_ID.clone()))
    };

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
        _ => None,
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    if state.done {
        return Task::none();
    }
    match message {
        Message::PinChanged(value) => {
            state.pin = value;
            state.mismatch = false;
            Task::none()
        }
        Message::RepeatChanged(value) => {
            state.repeat = value;
            state.mismatch = false;
            Task::none()
        }
        Message::ToggleReveal => {
            state.reveal = !state.reveal;
            Task::none()
        }
        Message::Confirm => match state.config.kind {
            DialogKind::Pin => {
                if state.config.repeat_label.is_some() && state.pin != state.repeat {
                    state.mismatch = true;
                    Task::none()
                } else {
                    let pin = std::mem::take(&mut state.pin);
                    state.finish(DialogResult::Pin(pin))
                }
            }
            DialogKind::Confirm { .. } | DialogKind::Message => {
                state.finish(DialogResult::Confirmed)
            }
        },
        Message::Decline => state.finish(DialogResult::Declined),
        Message::Cancel => state.finish(DialogResult::Cancelled),
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

fn header(config: &DialogConfig, animating: bool) -> Element<'_, Message> {
    let icon = container(
        canvas_widget(LockIcon { animating })
            .width(Length::Fixed(38.0))
            .height(Length::Fixed(38.0)),
    )
    .style(icon_style);

    row![icon, text(config.heading.clone()).size(17).color(TITLE)]
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
        section = section.push(
            progress_bar(0.0..=1.0, strength(&state.pin))
                .girth(Length::Fixed(3.0))
                .style(|_theme| progress_bar::Style {
                    background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06)),
                    bar: Background::Color(ACCENT),
                    border: Border {
                        radius: 2.0.into(),
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
    let mut input = text_input(placeholder, value)
        .secure(!reveal)
        .on_input(on_input)
        .font(MONO)
        .size(15)
        .padding(0)
        .width(Length::Fill)
        .style(borderless_field);
    if let Some(id) = id {
        input = input.id(id);
    }

    let mut inner = row![input].align_y(iced::Alignment::Center).spacing(10);

    if with_toggle {
        let label = if reveal { "Hide" } else { "Show" };
        inner = inner.push(
            button(text(label).size(13))
                .on_press(Message::ToggleReveal)
                .padding(0)
                .style(toggle_button),
        );
    }

    container(inner)
        .padding([11, 13])
        .width(Length::Fill)
        .style(field_style)
        .into()
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

struct LockIcon {
    animating: bool,
}

impl Program<Message> for LockIcon {
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
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: iced::Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let stroke = || Stroke::default().with_width(2.0).with_color(LOCK);

        let body =
            Path::rounded_rectangle(Point::new(10.0, 15.0), Size::new(18.0, 14.0), 4.0.into());
        frame.stroke(&body, stroke());

        let shackle = Path::new(|builder| {
            builder.arc(PathArc {
                center: Point::new(19.0, 15.0),
                radius: 5.5,
                start_angle: Radians(PI),
                end_angle: Radians(TAU),
            });
        });
        frame.stroke(&shackle, stroke());

        frame.fill(&Path::circle(Point::new(19.0, 21.5), 1.4), LOCK);

        vec![frame.into_geometry()]
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
            radius: 11.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn field_style(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(FIELD_BG)),
        border: Border {
            color: FIELD_BORDER,
            width: 1.0,
            radius: 9.0.into(),
        },
        ..Default::default()
    }
}

fn borderless_field(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => FIELD_BORDER_FOCUS,
        _ => Color::TRANSPARENT,
    };
    text_input::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            color: border_color,
            width: 0.0,
            radius: 0.0.into(),
        },
        icon: Color::TRANSPARENT,
        placeholder: PLACEHOLDER,
        value: TITLE,
        selection: Color { a: 0.35, ..ACCENT },
    }
}

fn pill_button(_theme: &Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered | button::Status::Pressed => {
            Color::from_rgba(0.36, 0.56, 0.96, 0.16)
        }
        _ => Color::from_rgba(0.36, 0.56, 0.96, 0.10),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: PILL_FG,
        border: Border {
            radius: 9.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn text_button(_theme: &Theme, status: button::Status) -> button::Style {
    let fg = match status {
        button::Status::Hovered | button::Status::Pressed => TITLE,
        _ => CANCEL_FG,
    };
    button::Style {
        background: None,
        text_color: fg,
        ..Default::default()
    }
}

fn toggle_button(_theme: &Theme, status: button::Status) -> button::Style {
    let fg = match status {
        button::Status::Hovered | button::Status::Pressed => TITLE,
        _ => SHOW,
    };
    button::Style {
        background: None,
        text_color: fg,
        ..Default::default()
    }
}

fn strength(pin: &str) -> f32 {
    if pin.is_empty() {
        return 0.0;
    }
    let len = pin.chars().count();
    let mut classes = 0u32;
    if pin.chars().any(|c| c.is_lowercase()) {
        classes += 1;
    }
    if pin.chars().any(|c| c.is_uppercase()) {
        classes += 1;
    }
    if pin.chars().any(|c| c.is_numeric()) {
        classes += 1;
    }
    if pin.chars().any(|c| !c.is_alphanumeric()) {
        classes += 1;
    }
    let length_score = (len as f32 / 16.0).min(1.0);
    let variety_score = classes as f32 / 4.0;
    (0.6 * length_score + 0.4 * variety_score).min(1.0)
}

pub struct LayerShell;

impl LayerShell {
    fn config(settings: &ProtoSettings, kind: DialogKind) -> DialogConfig {
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
            kind,
        }
    }
}

impl Frontend for LayerShell {
    fn get_pin(&mut self, settings: &ProtoSettings) -> PinOutcome {
        match run_dialog(Self::config(settings, DialogKind::Pin)) {
            DialogResult::Pin(pin) => PinOutcome::Entered(pin),
            _ => PinOutcome::Cancelled,
        }
    }

    fn confirm(&mut self, settings: &ProtoSettings, one_button: bool) -> ConfirmOutcome {
        match run_dialog(Self::config(settings, DialogKind::Confirm { one_button })) {
            DialogResult::Confirmed => ConfirmOutcome::Yes,
            DialogResult::Declined => ConfirmOutcome::No,
            _ => ConfirmOutcome::Cancelled,
        }
    }

    fn message(&mut self, settings: &ProtoSettings) {
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
