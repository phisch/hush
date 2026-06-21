use gpui::{div, prelude::*, px, relative, svg, AnyElement, Div, Stateful};
use theme::{theme, Element};

use crate::style::{apply, colors};

const CARD_WIDTH: f32 = 530.0;

pub(crate) fn backdrop() -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p(px(64.))
        .bg(theme().backdrop.base.background)
}

pub(crate) fn card(body: Vec<AnyElement>) -> Div {
    apply(div().flex().flex_col().w(px(CARD_WIDTH)), &theme().window.base).children(body)
}

pub(crate) fn header(title: &str) -> Div {
    let icon = &theme().icon.base;
    let glyph = apply(div().flex().items_center().justify_center(), icon).child(
        svg()
            .path("lock.svg")
            .size(px(icon.size))
            .text_color(icon.text),
    );
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(14.))
        .child(glyph)
        .child(apply(div(), &theme().title.base).child(title.to_string()))
}

pub(crate) fn banner(message: &str) -> Div {
    apply(div().w_full(), &theme().error.base).child(message.to_string())
}

pub(crate) fn info_line(line: &str) -> AnyElement {
    let label = &theme().description_label.base;
    let value = &theme().description_value.base;
    if let Some((name, text)) = line.split_once(": ") {
        if !name.is_empty() && name.len() <= 24 && !text.trim().is_empty() {
            return div()
                .flex()
                .flex_row()
                .gap(px(8.))
                .child(apply(div(), label).child(format!("{name}:")))
                .child(apply(div().w_full(), value).child(text.trim().to_string()))
                .into_any_element();
        }
    }
    apply(div().w_full(), value)
        .child(line.to_string())
        .into_any_element()
}

pub(crate) fn info_block<'a>(lines: impl Iterator<Item = &'a str>) -> Option<AnyElement> {
    let lines: Vec<AnyElement> = lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| info_line(l.trim()))
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(lines)
            .into_any_element(),
    )
}

pub(crate) fn hint_line(items: &[(&str, &str)]) -> Div {
    let key = &theme().hint_key.base;
    let word = &theme().hint_word.base;
    let mut line = div().flex().flex_row().items_center().gap(px(6.));
    for (i, (glyph, label)) in items.iter().enumerate() {
        if i > 0 {
            line = line.child(apply(div(), word).child("\u{00b7}"));
        }
        line = line
            .child(apply(div(), key).child(glyph.to_string()))
            .child(apply(div(), word).child(label.to_string()));
    }
    line
}

pub(crate) fn button(id: &'static str, label: String, element: &Element) -> Stateful<Div> {
    apply(div(), &element.base)
        .id(id)
        .cursor_pointer()
        .hover(colors(&element.hover))
        .active(colors(&element.active))
        .child(label)
}

pub(crate) fn strength_bar(char_count: usize) -> Div {
    let t = theme();
    let value = (char_count as f32 / 20.0).clamp(0.0, 1.0);
    let fill = if value < 0.4 {
        &t.strength_weak
    } else if value < 0.75 {
        &t.strength_medium
    } else {
        &t.strength_strong
    };
    let radius = px(t.strength.base.radius);
    apply(
        div().w_full().h(px(t.strength.base.size)).overflow_hidden(),
        &t.strength.base,
    )
    .child(
        div()
            .h_full()
            .w(relative(value))
            .rounded(radius)
            .bg(fill.base.background),
    )
}
