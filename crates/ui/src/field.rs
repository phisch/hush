use std::mem;
use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::{
    canvas, div, fill, point, prelude::*, px, size, App, Bounds, Context, CursorStyle,
    DispatchPhase, Entity, EventEmitter, FocusHandle, Font, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, ShapedLine, SharedString, TextRun,
    Window,
};
use theme::theme;
use zeroize::Zeroizing;

use crate::style::apply;

const MASKING_GLYPH: &str = "\u{2022}";
const BLINK_INTERVAL: Duration = Duration::from_millis(600);

pub(crate) struct FieldChanged;

pub(crate) struct Field {
    focus: FocusHandle,
    value: Zeroizing<String>,
    cursor: usize,
    anchor: usize,
    masked: bool,
    placeholder: SharedString,
    trailing_pad: Pixels,
    scroll: Pixels,
    blink_since: Instant,
    focused: bool,
    blinking: bool,
    selecting: bool,
}

impl Field {
    pub(crate) fn new(
        cx: &mut Context<Self>,
        masked: bool,
        placeholder: impl Into<SharedString>,
        trailing_pad: Pixels,
    ) -> Self {
        Self {
            focus: cx.focus_handle(),
            value: Zeroizing::default(),
            cursor: 0,
            anchor: 0,
            masked,
            placeholder: placeholder.into(),
            trailing_pad,
            scroll: px(0.),
            blink_since: Instant::now(),
            focused: false,
            blinking: false,
            selecting: false,
        }
    }

    pub(crate) fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }

    pub(crate) fn char_count(&self) -> usize {
        self.value.chars().count()
    }

    pub(crate) fn take_value(&mut self) -> Zeroizing<String> {
        self.cursor = 0;
        self.anchor = 0;
        mem::take(&mut self.value)
    }

    pub(crate) fn set_masked(&mut self, masked: bool) {
        self.masked = masked;
    }

    fn selection(&self) -> Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }

    fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    fn set_cursor(&mut self, pos: usize, extend: bool) -> bool {
        self.cursor = pos;
        if !extend {
            self.anchor = pos;
        }
        false
    }

    fn replace_selection(&mut self, text: &str) -> bool {
        let range = self.selection();
        self.value.replace_range(range.clone(), text);
        self.cursor = range.start + text.len();
        self.anchor = self.cursor;
        true
    }

    fn move_left(&mut self, word: bool, extend: bool) -> bool {
        let to = if self.has_selection() && !extend {
            self.selection().start
        } else if word {
            prev_word(&self.value, self.cursor)
        } else {
            prev_boundary(&self.value, self.cursor)
        };
        self.set_cursor(to, extend)
    }

    fn move_right(&mut self, word: bool, extend: bool) -> bool {
        let to = if self.has_selection() && !extend {
            self.selection().end
        } else if word {
            next_word(&self.value, self.cursor)
        } else {
            next_boundary(&self.value, self.cursor)
        };
        self.set_cursor(to, extend)
    }

    fn delete(&mut self, boundary: fn(&str, usize) -> usize) -> bool {
        if !self.has_selection() {
            self.anchor = boundary(&self.value, self.cursor);
        }
        self.replace_selection("")
    }

    fn on_key(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        let shift = k.modifiers.shift;
        let ctrl = k.modifiers.control;
        let edited = match k.key.as_str() {
            "left" => self.move_left(ctrl, shift),
            "right" => self.move_right(ctrl, shift),
            "home" => self.set_cursor(0, shift),
            "end" => self.set_cursor(self.value.len(), shift),
            "a" if ctrl => {
                self.set_cursor(0, false);
                self.set_cursor(self.value.len(), true)
            }
            "backspace" => self.delete(prev_boundary),
            "delete" => self.delete(next_boundary),
            "v" if ctrl => match cx.read_from_clipboard().and_then(|item| item.text()) {
                Some(text) => self.replace_selection(&text.replace(|c: char| c.is_control(), "")),
                None => return,
            },
            _ if ctrl => return,
            _ => match k.key_char.as_deref() {
                Some(ch) if !ch.contains(char::is_control) => self.replace_selection(ch),
                _ => return,
            },
        };
        self.blink_since = Instant::now();
        if edited {
            cx.emit(FieldChanged);
        }
        cx.notify();
    }

    fn display(&self) -> String {
        if self.masked {
            MASKING_GLYPH.repeat(self.char_count())
        } else {
            self.value.to_string()
        }
    }

    fn to_display(&self, value_byte: usize) -> usize {
        if self.masked {
            self.value[..value_byte].chars().count() * MASKING_GLYPH.len()
        } else {
            value_byte
        }
    }

    fn to_value(&self, display_byte: usize) -> usize {
        if !self.masked {
            return display_byte.min(self.value.len());
        }
        self.value
            .char_indices()
            .nth(display_byte / MASKING_GLYPH.len())
            .map_or(self.value.len(), |(i, _)| i)
    }

    fn start_blinking(&mut self, cx: &mut Context<Self>) {
        if mem::replace(&mut self.blinking, true) {
            return;
        }
        self.blink_since = Instant::now();
        cx.spawn(async move |this, cx| {
            loop {
                let Ok(wait) = this.update(cx, |f, _| {
                    let period = BLINK_INTERVAL.as_millis() as u64;
                    Duration::from_millis(
                        period - f.blink_since.elapsed().as_millis() as u64 % period,
                    )
                }) else {
                    break;
                };
                cx.background_executor().timer(wait).await;
                let alive = this.update(cx, |f, cx| {
                    if f.focused {
                        cx.notify();
                    }
                    f.focused
                });
                if !matches!(alive, Ok(true)) {
                    break;
                }
            }
            let _ = this.update(cx, |f, _| f.blinking = false);
        })
        .detach();
    }
}

impl EventEmitter<FieldChanged> for Field {}

impl Render for Field {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focused = self.focus.is_focused(window);
        if self.focused {
            self.start_blinking(cx);
        }
        let style = if self.focused {
            &theme().field.focus
        } else {
            &theme().field.base
        };
        let field = cx.entity();
        apply(
            div()
                .track_focus(&self.focus)
                .on_key_down(cx.listener(Self::on_key))
                .cursor(CursorStyle::IBeam)
                .flex()
                .items_center()
                .w_full()
                .overflow_hidden(),
            style,
        )
        .pr(px(style.padding.x) + self.trailing_pad)
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, cx| draw(&field, bounds, window, cx),
            )
            .w_full()
            .h(px(style.size * 1.3)),
        )
    }
}

fn draw(field: &Entity<Field>, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let t = &theme().field.base;
    let f = field.read(cx);
    let empty = f.value.is_empty();
    let (focused, prev_scroll, cursor_off) = (f.focused, f.scroll, f.to_display(f.cursor));
    let blink_on =
        (f.blink_since.elapsed().as_millis() / BLINK_INTERVAL.as_millis()).is_multiple_of(2);
    let selection = f.has_selection().then(|| {
        let r = f.selection();
        (f.to_display(r.start), f.to_display(r.end))
    });
    let (text, color) = if empty {
        (f.placeholder.clone(), t.placeholder)
    } else {
        (f.display().into(), t.text)
    };

    let run = TextRun {
        len: text.len(),
        font: field_font(window, &t.font),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text, px(t.size), &[run], None);

    let caret_x = if empty {
        px(0.)
    } else {
        line.x_for_index(cursor_off)
    };
    let scroll = sticky_scroll(prev_scroll, caret_x, line.width, bounds.size.width);
    if scroll != prev_scroll {
        field.update(cx, |f, _| f.scroll = scroll);
    }
    let origin = point(bounds.left() - scroll, bounds.top());

    let highlight = selection.map(|(start, end)| {
        Bounds::from_corners(
            point(origin.x + line.x_for_index(start), bounds.top()),
            point(origin.x + line.x_for_index(end), bounds.bottom()),
        )
    });

    window.paint_layer(bounds, |window| {
        if let Some(rect) = highlight {
            window.paint_quad(fill(rect, t.selection));
        }
        let _ = line.paint(origin, bounds.size.height, window, cx);
        if focused && blink_on {
            window.paint_quad(fill(
                Bounds::new(
                    point(origin.x + caret_x, bounds.top()),
                    size(px(2.), bounds.size.height),
                ),
                t.text,
            ));
        }
    });

    let hit = move |line: &ShapedLine, f: &Field, x: Pixels| {
        f.to_value(line.closest_index_for_x((x - bounds.left() + scroll).max(px(0.))))
    };

    let entity = field.clone();
    let click_line = line.clone();
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || event.button != MouseButton::Left
            || !bounds.contains(&event.position)
        {
            return;
        }
        window.focus(&entity.read(cx).focus);
        entity.update(cx, |f, cx| {
            let at = hit(&click_line, f, event.position.x);
            f.set_cursor(at, event.modifiers.shift);
            f.selecting = true;
            f.blink_since = Instant::now();
            cx.notify();
        });
    });

    let entity = field.clone();
    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
        if phase != DispatchPhase::Bubble || !entity.read(cx).selecting {
            return;
        }
        entity.update(cx, |f, cx| {
            f.cursor = hit(&line, f, event.position.x);
            cx.notify();
        });
    });

    let entity = field.clone();
    window.on_mouse_event(move |_event: &MouseUpEvent, phase, _window, cx| {
        if phase == DispatchPhase::Bubble {
            entity.update(cx, |f, cx| {
                if mem::take(&mut f.selecting) {
                    cx.notify();
                }
            });
        }
    });
}

fn sticky_scroll(prev: Pixels, caret: Pixels, content: Pixels, width: Pixels) -> Pixels {
    let margin = px(2.);
    let scroll = if caret - prev < margin {
        caret - margin
    } else if caret - prev > width - margin {
        caret - (width - margin)
    } else {
        prev
    };
    scroll.clamp(px(0.), (content - width + margin).max(px(0.)))
}

fn field_font(window: &Window, family: &theme::FontFamily) -> Font {
    let mut font = window.text_style().font();
    match family {
        theme::FontFamily::Default => {}
        theme::FontFamily::Monospace => font.family = "monospace".into(),
        theme::FontFamily::Named(name) => font.family = name.clone().into(),
    }
    font
}

fn prev_boundary(s: &str, i: usize) -> usize {
    s[..i].chars().next_back().map_or(0, |c| i - c.len_utf8())
}

fn next_boundary(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(i, |c| i + c.len_utf8())
}

fn prev_word(s: &str, i: usize) -> usize {
    let head = s[..i].trim_end();
    match head.rfind(char::is_whitespace) {
        Some(pos) => pos + s[pos..].chars().next().map_or(0, char::len_utf8),
        None => 0,
    }
}

fn next_word(s: &str, i: usize) -> usize {
    let start = i + (s[i..].len() - s[i..].trim_start().len());
    s[start..]
        .find(char::is_whitespace)
        .map_or(s.len(), |pos| start + pos)
}
