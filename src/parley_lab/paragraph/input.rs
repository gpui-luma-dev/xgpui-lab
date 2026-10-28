//! GPUI's existing UTF-16 input bridge over paragraph-local Parley geometry.
use super::*;
use gpui::{ClipboardItem, EntityInputHandler, UTF16Selection};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

fn byte(text: &str, utf16: usize) -> usize {
    let mut count = 0;
    for (index, ch) in text.char_indices() {
        if count + ch.len_utf16() > utf16 {
            return index;
        }
        count += ch.len_utf16();
    }
    text.len()
}
fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace(['\r', '\n', '\u{2029}'], "\u{2028}")
}
fn position(byte_index: usize) -> ParagraphPosition {
    ParagraphPosition { byte_index, affinity: TextAffinity::Downstream }
}

impl ParagraphLab {
    fn selection(&self) -> Range<usize> {
        self.anchor.byte_index.min(self.caret.byte_index)..self.anchor.byte_index.max(self.caret.byte_index)
    }
    fn bytes(&self, range: Range<usize>) -> Range<usize> {
        byte(&self.content, range.start)..byte(&self.content, range.end.max(range.start))
    }
    fn utf16(&self, range: Range<usize>) -> Range<usize> {
        self.content[..range.start].encode_utf16().count()..self.content[..range.end].encode_utf16().count()
    }
    fn replace(
        &mut self,
        range: Range<usize>,
        text: &str,
        selected: Option<Range<usize>>,
        marked: bool,
        cx: &mut Context<Self>,
    ) {
        let normalized = normalize(text);
        // Selection is relative to the original inserted UTF-16 text. Account for
        // CRLF normalization before converting its offsets into stored byte indices.
        let selected = selected
            .map(|r| {
                normalize(&text[..byte(text, r.start)]).len()..normalize(&text[..byte(text, r.end.max(r.start))]).len()
            })
            .unwrap_or(normalized.len()..normalized.len());
        self.content.replace_range(range.clone(), &normalized);
        self.marked = (marked && !normalized.is_empty()).then_some(range.start..range.start + normalized.len());
        self.anchor = position(range.start + selected.start);
        self.caret = position(range.start + selected.end);
        self.geometry = None; // Never answer IME geometry using a previous revision.
        self.preferred_x = None;
        self.reveal_caret = true;
        cx.notify();
    }
    pub(super) fn edit_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let key = event.keystroke.key.as_str();
        let modifiers = event.keystroke.modifiers;
        let command = if cfg!(target_os = "macos") {
            modifiers.platform
        } else {
            modifiers.control
        };
        if command {
            match key {
                "a" => {
                    self.anchor = position(0);
                    self.caret = position(self.content.len());
                    self.preferred_x = None;
                    self.reveal_caret = true;
                    cx.notify();
                }
                "c" | "x" => {
                    let range = self.selection();
                    if !range.is_empty() {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            self.content[range.clone()].replace('\u{2028}', "\n"),
                        ));
                        if key == "x" {
                            self.replace(range, "", None, false, cx);
                        }
                    }
                }
                "v" => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        self.replace(self.selection(), &text, None, false, cx);
                    }
                }
                _ => return false, // Leave other command chords to GPUI/platform bindings.
            }
            return true;
        }
        match key {
            "backspace" | "delete" => {
                let mut range = self.selection();
                if range.is_empty() {
                    let cursor = self.caret.byte_index;
                    if key == "backspace" {
                        range.start =
                            self.content.grapheme_indices(true).rev().find(|(i, _)| *i < cursor).map_or(0, |(i, _)| i);
                    } else {
                        range.end = self
                            .content
                            .grapheme_indices(true)
                            .find(|(i, _)| *i > cursor)
                            .map_or(self.content.len(), |(i, _)| i);
                    }
                }
                self.replace(range, "", None, false, cx);
            }
            "enter" => self.replace(self.selection(), "\u{2028}", None, false, cx),
            "home" | "end" => {
                self.caret = position(if key == "home" { 0 } else { self.content.len() });
                if !modifiers.shift {
                    self.anchor = self.caret;
                }
                self.preferred_x = None;
                self.reveal_caret = true;
                cx.notify();
            }
            "up" | "down" => {
                if let Ok(layout) = self.layout(window)
                    && let Ok(rect) = layout.caret_bounds(self.caret)
                {
                    let x = self.preferred_x.unwrap_or(rect.left());
                    let y = if key == "up" {
                        rect.top() - px(1.0)
                    } else {
                        rect.bottom() + px(1.0)
                    };
                    if let Ok(next) = layout.hit_test(gpui::point(x, y)) {
                        self.caret = next;
                        self.preferred_x = Some(x);
                        if !modifiers.shift {
                            self.anchor = next;
                        }
                        self.reveal_caret = true;
                        cx.notify();
                    }
                }
            }
            _ => return false,
        }
        true
    }
}
impl EntityInputHandler for ParagraphLab {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.bytes(range);
        *actual = Some(self.utf16(range.clone()));
        Some(self.content[range].to_owned())
    }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        Some(self.content.encode_utf16().count())
    }
    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.utf16(self.selection()),
            reversed: self.caret.byte_index < self.anchor.byte_index,
        })
    }
    fn set_selected_text_range(&mut self, range: Range<usize>, _: &mut Window, cx: &mut Context<Self>) {
        let range = self.bytes(range);
        self.anchor = position(range.start);
        self.caret = position(range.end);
        self.reveal_caret = true;
        cx.notify();
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.clone().map(|r| self.utf16(r))
    }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|r| self.bytes(r)).or(self.marked.clone()).unwrap_or_else(|| self.selection());
        self.replace(range, text, None, false, cx);
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range.map(|r| self.bytes(r)).or(self.marked.clone()).unwrap_or_else(|| self.selection());
        self.replace(range, text, selected, true, cx);
    }
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let (bounds, layout) = self.geometry.as_ref()?;
        if layout.source() != self.content {
            return None;
        }
        let range = self.bytes(range);
        let start = if range.start == self.caret.byte_index {
            self.caret
        } else {
            position(range.start)
        };
        let caret = layout.caret_bounds(start).ok()?;
        let rect = if range.is_empty() {
            caret
        } else {
            let rects = layout.selection_bounds(start, position(range.end)).ok()?;
            rects
                .iter()
                .find(|rect| {
                    rect.top() <= caret.center().y
                        && rect.bottom() >= caret.center().y
                        && rect.left() <= caret.left()
                        && rect.right() >= caret.left()
                })
                .or(rects.first())
                .copied()
                .unwrap_or(caret)
        };
        Some(Bounds::new(bounds.origin + rect.origin, rect.size))
    }
    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let (bounds, layout) = self.geometry.as_ref()?;
        if layout.source() != self.content {
            return None;
        }
        let hit = layout.hit_test(bounds.localize(&point)?).ok()?;
        Some(self.content[..hit.byte_index].encode_utf16().count())
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use gpui::{ElementInputHandler, InputHandler, TestAppContext, VisualTestContext, point, size};

    #[test]
    fn paragraph_ime_edits_utf16_and_wrapped_candidate_geometry() -> Result<()> {
        let mut cx = TestAppContext::single();
        cx.update(|cx| gpui_luma::init(cx).unwrap());
        let window = cx.open_window(size(px(580.0), px(1000.0)), |_, cx| {
            ParagraphLab::new(
                crate::parley_lab::Engine::new().unwrap(),
                vec![Timing { first: 0.0, warm: 0.0, reflow: 0.0 }; TEXTS.len()],
                crate::theme::default_look(),
                cx,
            )
        });
        let lab = window.root(&mut cx).unwrap();
        let mut visual = VisualTestContext::from_window(window.into(), &cx);
        let draw = |visual: &mut VisualTestContext| {
            visual.draw(point(px(80.0), px(60.0)), size(px(580.0), px(1000.0)), |_, _| {
                div().size_full().child(lab.clone())
            });
        };
        draw(&mut visual);
        visual.update(|window, cx| {
            window.focus(&lab.focus_handle(cx), cx);
            let bounds = lab.read(cx).geometry.as_ref().unwrap().0;
            let mut handler = ElementInputHandler::new(bounds, lab.clone());
            handler.replace_text_in_range(Some(0..usize::MAX), "abc אבג مرحبا office words wrap here\n", window, cx);
            let end = handler.text_length_utf16(window, cx).unwrap();
            handler.replace_and_mark_text_in_range(None, "𝄞e\u{301}", Some(2..4), window, cx);
            assert_eq!(handler.marked_text_range(window, cx), Some(end..end + 4));
            assert_eq!(handler.selected_text_range(false, window, cx).unwrap().range, end + 2..end + 4);
            assert!(handler.bounds_for_range(end..end, window, cx).is_none());
            assert!(handler.character_index_for_point(bounds.center(), window, cx).is_none());
        });
        draw(&mut visual); // Paint actual preedit and its fragment underlines.
        visual.update(|window, cx| {
            let (bounds, layout) = lab.read(cx).geometry.clone().unwrap();
            let mut handler = ElementInputHandler::new(bounds, lab.clone());
            let marked = handler.marked_text_range(window, cx).unwrap();
            let rect = handler.bounds_for_range(marked.start..marked.start, window, cx).unwrap();
            assert!(rect.top() > bounds.top()); // Composition is after an explicit break.
            assert!(rect.left() >= bounds.left() && rect.right() <= bounds.right() + px(1.0));
            let expected = layout.caret_bounds(position(byte(layout.source(), marked.start))).unwrap();
            assert_eq!(rect.origin, bounds.origin + expected.origin);
            let hit = handler.character_index_for_point(rect.center(), window, cx).unwrap();
            assert!(hit >= marked.start && hit <= marked.end);
            // Candidate changes replace the marked span, preserving surrounding
            // text and interpreting the new selection relative to each candidate.
            let prefix = lab.read(cx).content[..byte(&lab.read(cx).content, marked.start)].to_owned();
            handler.replace_and_mark_text_in_range(None, "אבג", Some(1..2), window, cx);
            assert_eq!(handler.marked_text_range(window, cx), Some(marked.start..marked.start + 3));
            assert_eq!(
                handler.selected_text_range(false, window, cx).unwrap().range,
                marked.start + 1..marked.start + 2
            );
            handler.replace_and_mark_text_in_range(None, "אב", Some(0..2), window, cx);
            assert_eq!(handler.marked_text_range(window, cx), Some(marked.start..marked.start + 2));
            assert_eq!(lab.read(cx).content, format!("{prefix}אב"));
            handler.replace_text_in_range(None, "אב", window, cx);
            assert_eq!(handler.marked_text_range(window, cx), None);
            handler.replace_and_mark_text_in_range(None, "مرحبا", Some(5..5), window, cx);
            handler.replace_and_mark_text_in_range(None, "", Some(0..0), window, cx);
            assert_eq!(handler.marked_text_range(window, cx), None);
            assert!(lab.read(cx).content.ends_with("אב"));
            // New selection offsets are relative to inserted text even with CRLF folding.
            handler.replace_and_mark_text_in_range(Some(0..usize::MAX), "x\r\ny", Some(3..4), window, cx);
            assert_eq!(handler.selected_text_range(false, window, cx).unwrap().range, 2..3);
            assert_eq!(lab.read(cx).content, "x\u{2028}y");
            handler.unmark_text(window, cx);
            handler.replace_text_in_range(Some(0..usize::MAX), "", window, cx);
        });
        draw(&mut visual);
        visual.simulate_keystrokes("a enter b");
        assert_eq!(visual.update(|_, cx| lab.read(cx).content.clone()), "a\u{2028}b");
        visual.update(|window, cx| {
            let bounds = lab.read(cx).geometry.as_ref().unwrap().0;
            let mut handler = ElementInputHandler::new(bounds, lab.clone());
            handler.replace_text_in_range(Some(0..usize::MAX), "e\u{301}", window, cx);
        });
        draw(&mut visual);
        visual.simulate_keystrokes("backspace");
        assert!(visual.update(|_, cx| lab.read(cx).content.is_empty()));
        visual.simulate_keystrokes("a left b"); // Navigation works before a repaint after edits.
        assert_eq!(visual.update(|_, cx| lab.read(cx).content.clone()), "ba");
        let command = if cfg!(target_os = "macos") { "cmd" } else { "ctrl" };
        visual.simulate_keystrokes(&format!("{command}-a {command}-c backspace {command}-v"));
        assert_eq!(visual.update(|_, cx| lab.read(cx).content.clone()), "ba");
        Ok(())
    }
}
