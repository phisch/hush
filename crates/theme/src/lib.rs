use std::path::PathBuf;
use std::sync::OnceLock;

use iced_core::{Color, Font};
use kdl::{KdlDocument, KdlNode, KdlValue};

const fn rgba(value: u32) -> Color {
    Color {
        r: ((value >> 24) & 0xFF) as f32 / 255.0,
        g: ((value >> 16) & 0xFF) as f32 / 255.0,
        b: ((value >> 8) & 0xFF) as f32 / 255.0,
        a: (value & 0xFF) as f32 / 255.0,
    }
}

const LOCK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" height="40px" viewBox="0 -960 960 960" width="40px" fill="#e3e3e3"><path d="M226.67-80q-27.5 0-47.09-19.58Q160-119.17 160-146.67v-422.66q0-27.5 19.58-47.09Q199.17-636 226.67-636h60v-90.67q0-80.23 56.57-136.78T480.07-920q80.26 0 136.76 56.55 56.5 56.55 56.5 136.78V-636h60q27.5 0 47.09 19.58Q800-596.83 800-569.33v422.66q0 27.5-19.58 47.09Q760.83-80 733.33-80H226.67Zm0-66.67h506.66v-422.66H226.67v422.66Zm308.5-155.85Q558-325.04 558-356.67q0-31-22.95-55.16Q512.11-436 479.89-436t-55.06 24.17Q402-387.67 402-356.33q0 31.33 22.95 53.83 22.94 22.5 55.16 22.5t55.06-22.52ZM353.33-636h253.34v-90.67q0-52.77-36.92-89.72-36.93-36.94-89.67-36.94-52.75 0-89.75 36.94-37 36.95-37 89.72V-636ZM226.67-146.67v-422.66 422.66Z"/></svg>"##;

const EYE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="#e3e3e3"><path d="M599-361q49-49 49-119t-49-119q-49-49-119-49t-119 49q-49 49-49 119t49 119q49 49 119 49t119-49Zm-187-51q-28-28-28-68t28-68q28-28 68-28t68 28q28 28 28 68t-28 68q-28 28-68 28t-68-28ZM220-270.5Q103-349 48-480q55-131 172-209.5T480-768q143 0 260 78.5T912-480q-55 131-172 209.5T480-192q-143 0-260-78.5ZM480-480Zm207 158q95-58 146-158-51-100-146-158t-207-58q-112 0-207 58T127-480q51 100 146 158t207 58q112 0 207-58Z"/></svg>"##;

const EYE_OFF_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" height="20px" viewBox="0 -960 960 960" width="20px" fill="#e3e3e3"><path d="m637-425-62-62q4-38-23-65.5T487-576l-62-62q13-5 27-7.5t28-2.5q70 0 119 49t49 119q0 14-2.5 28t-8.5 27Zm133 133-52-52q36-28 65.5-61.5T833-480q-49-101-144.5-158.5T480-696q-26 0-51 3t-49 10l-58-58q38-15 77.5-21t80.5-6q143 0 261.5 77.5T912-480q-22 57-58.5 103.5T770-292Zm-2 202L638-220q-38 14-77.5 21t-80.5 7q-143 0-261.5-77.5T48-480q22-57 58-104t84-85L90-769l51-51 678 679-51 51ZM241-617q-35 28-65 61.5T127-480q49 101 144.5 158.5T480-264q26 0 51-3.5t50-9.5l-45-45q-14 5-28 7.5t-28 2.5q-70 0-119-49t-49-119q0-14 3.5-28t6.5-28l-81-81Zm287 89Zm-96 96Z"/></svg>"##;

#[derive(Clone, Debug, PartialEq)]
pub struct FontStyle {
    pub family: Font,
    pub size: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Padding {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BorderStyle {
    pub color: Color,
    pub size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldPaint {
    pub color: Color,
    pub placeholder: Color,
    pub selection: Color,
    pub background: Color,
    pub border: BorderStyle,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ButtonPaint {
    pub color: Color,
    pub background: Color,
    pub border: BorderStyle,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CheckboxPaint {
    pub color: Color,
    pub check: Color,
    pub background: Color,
    pub border: BorderStyle,
    pub radius: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShadowStyle {
    pub color: Color,
    pub blur: f32,
    pub offset: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Backdrop {
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub background: Color,
    pub border: BorderStyle,
    pub radius: f32,
    pub shadow: ShadowStyle,
    pub width: f32,
    pub padding: Padding,
    pub spacing: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TitleIcon {
    pub svg: String,
    pub color: Color,
    pub background: Color,
    pub border: BorderStyle,
    pub radius: f32,
    pub size: f32,
    pub padding: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Title {
    pub font: FontStyle,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub font: FontStyle,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Description {
    pub label: Text,
    pub value: Text,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub font: FontStyle,
    pub color: Color,
    pub background: Color,
    pub border: BorderStyle,
    pub radius: f32,
    pub padding: Padding,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub font: FontStyle,
    pub padding: Padding,
    pub paint: FieldPaint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Reveal {
    pub color: Color,
    pub size: f32,
    pub eye: String,
    pub eye_off: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Strength {
    pub track: Color,
    pub weak: Color,
    pub medium: Color,
    pub strong: Color,
    pub border: BorderStyle,
    pub radius: f32,
    pub height: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Checkbox {
    pub font: FontStyle,
    pub box_size: f32,
    pub spacing: f32,
    pub paint: CheckboxPaint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub font: FontStyle,
    pub padding: Padding,
    pub paint: ButtonPaint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hint {
    pub key: Text,
    pub word: Text,
    pub spacing: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub backdrop: Backdrop,
    pub window: Window,
    pub title_icon: TitleIcon,
    pub title: Title,
    pub description: Description,
    pub error: Error,
    pub field: Field,
    pub field_focus: FieldPaint,
    pub reveal: Reveal,
    pub strength: Strength,
    pub checkbox: Checkbox,
    pub checkbox_checked: CheckboxPaint,
    pub confirm: Button,
    pub confirm_hover: ButtonPaint,
    pub cancel: Button,
    pub cancel_hover: ButtonPaint,
    pub hint: Hint,
}

fn no_border() -> BorderStyle {
    BorderStyle {
        color: rgba(0x00000000),
        size: 0.0,
    }
}

impl Default for Theme {
    fn default() -> Self {
        let field = Field {
            font: FontStyle {
                family: Font::MONOSPACE,
                size: 15.0,
            },
            padding: Padding { x: 13.0, y: 11.0 },
            paint: FieldPaint {
                color: rgba(0xf0f0f2ff),
                placeholder: rgba(0x6b6e78ff),
                selection: rgba(0x59001a59),
                background: rgba(0x0f0f0fff),
                border: BorderStyle {
                    color: rgba(0x272727ff),
                    size: 1.0,
                },
                radius: 8.0,
            },
        };
        let field_focus = FieldPaint {
            background: rgba(0x1d1d1dff),
            ..field.paint.clone()
        };

        let checkbox = Checkbox {
            font: FontStyle {
                family: Font::DEFAULT,
                size: 13.0,
            },
            box_size: 16.0,
            spacing: 10.0,
            paint: CheckboxPaint {
                color: rgba(0x9ea1ab66),
                check: rgba(0xffffffff),
                background: rgba(0x0f0f0fff),
                border: BorderStyle {
                    color: rgba(0x272727ff),
                    size: 1.0,
                },
                radius: 4.0,
            },
        };
        let checkbox_checked = CheckboxPaint {
            color: rgba(0x9ea1abff),
            ..checkbox.paint.clone()
        };

        let confirm = Button {
            font: FontStyle {
                family: Font::DEFAULT,
                size: 13.0,
            },
            padding: Padding { x: 18.0, y: 9.0 },
            paint: ButtonPaint {
                color: rgba(0xFFFFFF80),
                background: rgba(0x4D00FF50),
                border: no_border(),
                radius: 6.0,
            },
        };
        let confirm_hover = ButtonPaint {
            background: rgba(0x4D00FF90),
            ..confirm.paint.clone()
        };

        let cancel = Button {
            font: FontStyle {
                family: Font::DEFAULT,
                size: 13.0,
            },
            padding: Padding { x: 12.0, y: 9.0 },
            paint: ButtonPaint {
                color: rgba(0x8f8f8fff),
                background: rgba(0xffffff02),
                border: no_border(),
                radius: 6.0,
            },
        };
        let cancel_hover = ButtonPaint {
            background: rgba(0xffffff03),
            ..cancel.paint.clone()
        };

        Theme {
            backdrop: Backdrop {
                color: rgba(0x000000A0),
            },
            window: Window {
                background: rgba(0x121212ff),
                border: BorderStyle {
                    color: rgba(0x212121ff),
                    size: 2.0,
                },
                radius: 16.0,
                shadow: ShadowStyle {
                    color: rgba(0x00000080),
                    blur: 40.0,
                    offset: 12.0,
                },
                width: 530.0,
                padding: Padding { x: 28.0, y: 26.0 },
                spacing: 20.0,
            },
            title_icon: TitleIcon {
                svg: LOCK_SVG.to_string(),
                color: rgba(0xDBCBFFFF),
                background: rgba(0x4D00FF50),
                border: no_border(),
                radius: 8.0,
                size: 24.0,
                padding: 10.0,
            },
            title: Title {
                font: FontStyle {
                    family: Font::with_name("Inter"),
                    size: 24.0,
                },
                color: rgba(0xf0f0f2ff),
            },
            description: Description {
                label: Text {
                    font: FontStyle {
                        family: Font::DEFAULT,
                        size: 13.0,
                    },
                    color: rgba(0x737580ff),
                },
                value: Text {
                    font: FontStyle {
                        family: Font::MONOSPACE,
                        size: 13.0,
                    },
                    color: rgba(0x9ea1abff),
                },
            },
            error: Error {
                font: FontStyle {
                    family: Font::DEFAULT,
                    size: 13.0,
                },
                color: rgba(0xeb7578ff),
                background: rgba(0xb01d3606),
                border: no_border(),
                radius: 6.0,
                padding: Padding { x: 12.0, y: 8.0 },
            },
            field,
            field_focus,
            reveal: Reveal {
                color: rgba(0x999ca8ff),
                size: 20.0,
                eye: EYE_SVG.to_string(),
                eye_off: EYE_OFF_SVG.to_string(),
            },
            strength: Strength {
                track: rgba(0x222222ff),
                weak: rgba(0xe5484dff),
                medium: rgba(0xe6b340ff),
                strong: rgba(0x5cc86bff),
                border: no_border(),
                radius: 3.0,
                height: 6.0,
            },
            checkbox,
            checkbox_checked,
            confirm,
            confirm_hover,
            cancel,
            cancel_hover,
            hint: Hint {
                key: Text {
                    font: FontStyle {
                        family: Font::MONOSPACE,
                        size: 12.0,
                    },
                    color: rgba(0xa5a5a5ff),
                },
                word: Text {
                    font: FontStyle {
                        family: Font::DEFAULT,
                        size: 12.0,
                    },
                    color: rgba(0x393939ff),
                },
                spacing: 6.0,
            },
        }
    }
}

static THEME: OnceLock<Theme> = OnceLock::new();

pub fn theme() -> &'static Theme {
    THEME.get_or_init(Theme::load)
}

impl Theme {
    fn load() -> Theme {
        let mut theme = Theme::default();
        let Some(text) = config_path().and_then(|path| std::fs::read_to_string(path).ok()) else {
            return theme;
        };
        match KdlDocument::parse(&text) {
            Ok(doc) => read_theme(&doc, &mut theme),
            Err(error) => eprintln!("psst: ignoring invalid theme: {error}"),
        }
        theme
    }
}

fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("psst").join("theme.kdl"))
}

fn parse_hex(text: &str) -> Option<Color> {
    let text = text.strip_prefix('#').unwrap_or(text).trim();
    let bytes = match text.len() {
        3 | 4 => hex_bytes(&text.chars().flat_map(|c| [c, c]).collect::<String>())?,
        6 | 8 => hex_bytes(text)?,
        _ => return None,
    };
    let alpha = bytes.get(3).copied().unwrap_or(255);
    Some(Color::from_rgba8(
        bytes[0],
        bytes[1],
        bytes[2],
        alpha as f32 / 255.0,
    ))
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

fn parse_font(name: &str) -> Font {
    match name.trim().to_ascii_lowercase().as_str() {
        "monospace" | "mono" => Font::MONOSPACE,
        "" | "default" | "sans" | "sans-serif" => Font::DEFAULT,
        _ => Font::with_name(Box::leak(name.trim().to_string().into_boxed_str())),
    }
}

fn child<'a>(parent: &'a KdlNode, name: &str) -> Option<&'a KdlNode> {
    parent
        .children()?
        .nodes()
        .iter()
        .find(|node| node.name().value() == name)
}

fn arg(node: &KdlNode) -> Option<&KdlValue> {
    node.entries()
        .iter()
        .find(|entry| entry.name().is_none())
        .map(|entry| entry.value())
}

fn set_color(parent: &KdlNode, name: &str, target: &mut Color) {
    if let Some(value) = child(parent, name).and_then(arg) {
        match value.as_string().and_then(parse_hex) {
            Some(color) => *target = color,
            None => eprintln!("psst: theme `{name}`: expected a color like \"#1d1d1d\""),
        }
    }
}

fn set_num(parent: &KdlNode, name: &str, target: &mut f32) {
    if let Some(value) = child(parent, name).and_then(arg) {
        if let Some(number) = value
            .as_float()
            .map(|n| n as f32)
            .or_else(|| value.as_integer().map(|n| n as f32))
        {
            *target = number;
        }
    }
}

fn set_string(parent: &KdlNode, name: &str, target: &mut String) {
    if let Some(text) = child(parent, name)
        .and_then(arg)
        .and_then(KdlValue::as_string)
    {
        *target = text.to_string();
    }
}

fn set_padding(parent: &KdlNode, target: &mut Padding) {
    if let Some(node) = child(parent, "padding") {
        set_num(node, "x", &mut target.x);
        set_num(node, "y", &mut target.y);
    }
}

fn set_border(parent: &KdlNode, target: &mut BorderStyle) {
    if let Some(node) = child(parent, "border") {
        set_color(node, "color", &mut target.color);
        set_num(node, "size", &mut target.size);
    }
}

fn read_font(parent: &KdlNode, target: &mut FontStyle) {
    if let Some(node) = child(parent, "font") {
        if let Some(text) = child(node, "family")
            .and_then(arg)
            .and_then(KdlValue::as_string)
        {
            target.family = parse_font(text);
        }
        set_num(node, "size", &mut target.size);
    }
}

fn layout(parent: &KdlNode) -> Option<&KdlNode> {
    child(parent, "layout")
}

fn read_text(parent: &KdlNode, name: &str, target: &mut Text) {
    if let Some(node) = child(parent, name) {
        if let Some(l) = layout(node) {
            read_font(l, &mut target.font);
        }
        set_color(node, "color", &mut target.color);
    }
}

fn read_field_paint(node: &KdlNode, target: &mut FieldPaint) {
    set_color(node, "color", &mut target.color);
    set_color(node, "placeholder", &mut target.placeholder);
    set_color(node, "selection", &mut target.selection);
    set_color(node, "background", &mut target.background);
    set_border(node, &mut target.border);
    set_num(node, "radius", &mut target.radius);
}

fn read_button_paint(node: &KdlNode, target: &mut ButtonPaint) {
    set_color(node, "color", &mut target.color);
    set_color(node, "background", &mut target.background);
    set_border(node, &mut target.border);
    set_num(node, "radius", &mut target.radius);
}

fn read_checkbox_paint(node: &KdlNode, target: &mut CheckboxPaint) {
    set_color(node, "color", &mut target.color);
    set_color(node, "check", &mut target.check);
    set_color(node, "background", &mut target.background);
    set_border(node, &mut target.border);
    set_num(node, "radius", &mut target.radius);
}

fn read_theme(doc: &KdlDocument, theme: &mut Theme) {
    let Some(backdrop) = doc.nodes().iter().find(|n| n.name().value() == "Backdrop") else {
        return;
    };
    set_color(backdrop, "color", &mut theme.backdrop.color);

    let Some(window) = child(backdrop, "Window") else {
        return;
    };
    set_color(window, "background", &mut theme.window.background);
    set_border(window, &mut theme.window.border);
    set_num(window, "radius", &mut theme.window.radius);
    if let Some(shadow) = child(window, "shadow") {
        set_color(shadow, "color", &mut theme.window.shadow.color);
        set_num(shadow, "blur", &mut theme.window.shadow.blur);
        set_num(shadow, "offset", &mut theme.window.shadow.offset);
    }
    if let Some(l) = layout(window) {
        set_num(l, "width", &mut theme.window.width);
        set_num(l, "spacing", &mut theme.window.spacing);
        set_padding(l, &mut theme.window.padding);
    }

    if let Some(node) = child(window, "TitleIcon") {
        set_string(node, "svg", &mut theme.title_icon.svg);
        set_color(node, "color", &mut theme.title_icon.color);
        set_color(node, "background", &mut theme.title_icon.background);
        set_border(node, &mut theme.title_icon.border);
        set_num(node, "radius", &mut theme.title_icon.radius);
        if let Some(l) = layout(node) {
            set_num(l, "size", &mut theme.title_icon.size);
            set_num(l, "padding", &mut theme.title_icon.padding);
        }
    }

    if let Some(node) = child(window, "Title") {
        if let Some(l) = layout(node) {
            read_font(l, &mut theme.title.font);
        }
        set_color(node, "color", &mut theme.title.color);
    }

    if let Some(node) = child(window, "Description") {
        read_text(node, "Label", &mut theme.description.label);
        read_text(node, "Value", &mut theme.description.value);
    }

    if let Some(node) = child(window, "Error") {
        set_color(node, "color", &mut theme.error.color);
        set_color(node, "background", &mut theme.error.background);
        set_border(node, &mut theme.error.border);
        set_num(node, "radius", &mut theme.error.radius);
        if let Some(l) = layout(node) {
            read_font(l, &mut theme.error.font);
            set_padding(l, &mut theme.error.padding);
        }
    }

    if let Some(node) = child(window, "Field") {
        read_field_paint(node, &mut theme.field.paint);
        if let Some(l) = layout(node) {
            read_font(l, &mut theme.field.font);
            set_padding(l, &mut theme.field.padding);
        }
        if let Some(focus) = child(node, ":focus") {
            read_field_paint(focus, &mut theme.field_focus);
        }
        if let Some(reveal) = child(node, "Reveal") {
            set_color(reveal, "color", &mut theme.reveal.color);
            set_string(reveal, "eye", &mut theme.reveal.eye);
            set_string(reveal, "eye-off", &mut theme.reveal.eye_off);
            if let Some(l) = layout(reveal) {
                set_num(l, "size", &mut theme.reveal.size);
            }
        }
        if let Some(strength) = child(node, "Strength") {
            set_color(strength, "track", &mut theme.strength.track);
            set_color(strength, "weak", &mut theme.strength.weak);
            set_color(strength, "medium", &mut theme.strength.medium);
            set_color(strength, "strong", &mut theme.strength.strong);
            set_border(strength, &mut theme.strength.border);
            set_num(strength, "radius", &mut theme.strength.radius);
            if let Some(l) = layout(strength) {
                set_num(l, "height", &mut theme.strength.height);
            }
        }
    }

    if let Some(node) = child(window, "Checkbox") {
        read_checkbox_paint(node, &mut theme.checkbox.paint);
        if let Some(l) = layout(node) {
            read_font(l, &mut theme.checkbox.font);
            set_num(l, "box", &mut theme.checkbox.box_size);
            set_num(l, "spacing", &mut theme.checkbox.spacing);
        }
        if let Some(checked) = child(node, ":checked") {
            read_checkbox_paint(checked, &mut theme.checkbox_checked);
        }
    }

    read_button(
        window,
        "Confirm",
        &mut theme.confirm,
        &mut theme.confirm_hover,
    );
    read_button(window, "Cancel", &mut theme.cancel, &mut theme.cancel_hover);

    if let Some(node) = child(window, "Hint") {
        read_text(node, "Key", &mut theme.hint.key);
        read_text(node, "Word", &mut theme.hint.word);
        if let Some(l) = layout(node) {
            set_num(l, "spacing", &mut theme.hint.spacing);
        }
    }
}

fn read_button(window: &KdlNode, name: &str, base: &mut Button, hover: &mut ButtonPaint) {
    if let Some(node) = child(window, name) {
        read_button_paint(node, &mut base.paint);
        if let Some(l) = layout(node) {
            read_font(l, &mut base.font);
            set_padding(l, &mut base.padding);
        }
        if let Some(node) = child(node, ":hover") {
            read_button_paint(node, hover);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn default_theme() -> &'static str {
        include_str!("default-theme.kdl")
    }

    fn read(text: &str) -> Theme {
        let mut theme = Theme::default();
        let doc = KdlDocument::parse(text).expect("parses");
        read_theme(&doc, &mut theme);
        theme
    }

    #[test]
    fn default_template_parses() {
        KdlDocument::parse(default_theme()).expect("default theme parses");
    }

    #[test]
    fn default_template_loads_without_warnings() {
        let mut theme = Theme::default();
        let doc = KdlDocument::parse(default_theme()).expect("parses");
        read_theme(&doc, &mut theme);
        assert_eq!(theme.field.paint.background, parse_hex("#0f0f0f").unwrap());
        assert_eq!(theme.field.font.size, 15.0);
        assert_eq!(theme.field_focus.background, parse_hex("#1d1d1d").unwrap());
    }

    #[test]
    fn font_lives_in_layout_color_is_appearance() {
        let theme = read(
            r##"Backdrop { Window { Title { layout { font { size 30; family "serif" } }; color "#abcdef" } } }"##,
        );
        assert_eq!(theme.title.font.size, 30.0);
        assert_eq!(theme.title.color, parse_hex("#abcdef").unwrap());
    }

    #[test]
    fn layout_props_are_ignored_inside_a_state() {
        let theme = read(
            r##"Backdrop { Window { Field { layout { font { size 15 } }; :focus { background "#abcdef"; layout { font { size 99 } } } } } }"##,
        );
        assert_eq!(theme.field.font.size, 15.0);
        assert_eq!(theme.field_focus.background, parse_hex("#abcdef").unwrap());
    }

    #[test]
    fn state_overrides_appearance_and_keeps_other_defaults() {
        let theme = read(
            r##"Backdrop { Window { Field { color "#111111"; :focus { background "#abcdef" } } } }"##,
        );
        assert_eq!(theme.field.paint.color, parse_hex("#111111").unwrap());
        assert_eq!(theme.field_focus.background, parse_hex("#abcdef").unwrap());
        assert_eq!(theme.field_focus.color, Theme::default().field_focus.color);
    }

    #[test]
    fn hint_word_family_is_settable() {
        let theme = read(
            r##"Backdrop { Window { Hint { Word { layout { font { family "serif" } } } } } }"##,
        );
        assert_eq!(theme.hint.word.font.family, Font::with_name("serif"));
    }

    #[test]
    fn hex_parses_rgb_rgba_and_shorthand() {
        assert_eq!(
            parse_hex("#ffffff"),
            Some(Color::from_rgba8(255, 255, 255, 1.0))
        );
        assert_eq!(parse_hex("#000"), Some(Color::from_rgba8(0, 0, 0, 1.0)));
        assert_eq!(parse_hex("#00000080").unwrap().a, 128.0 / 255.0);
        assert_eq!(parse_hex("nope"), None);
    }
}
