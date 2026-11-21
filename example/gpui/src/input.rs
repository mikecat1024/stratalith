use super::*;
use text::byte_for_utf16;

/// The OS may replace marked text repeatedly. Keep it outside the engine until
/// commit so an IME conversion occupies a single undo step.
pub(super) struct Composition {
    pub target: Range<usize>,
    pub text: String,
    pub selected: Range<usize>,
}

impl Editor {
    fn plain_text(&self) -> String {
        self.rows
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn input_text(&self) -> String {
        let mut text = self.plain_text();
        if let Some(mark) = &self.composition {
            text.replace_range(mark.target.clone(), &mark.text);
        }
        text
    }

    pub(super) fn global_byte(&self, pos: BlockPosition) -> usize {
        let index = self.row_index(pos);
        self.rows[index].start + self.rows[index].boundaries[pos.offset]
    }

    fn position_at_byte(&self, mut byte: usize) -> BlockPosition {
        for row in &self.rows {
            if byte <= row.text.len() {
                return row.position(byte);
            }
            byte -= row.text.len() + 1;
        }
        self.rows.last().unwrap().position(usize::MAX)
    }

    pub(super) fn selected_bytes(&self) -> Range<usize> {
        let selection = self.engine.selection();
        let a = self.global_byte(selection.anchor);
        let b = self.global_byte(selection.focus);
        a.min(b)..a.max(b)
    }

    fn replacement(&self, range: Option<Range<usize>>, new: &str) -> (Range<usize>, String, usize) {
        let text = self.input_text();
        let requested = range.map(|r| byte_for_utf16(&text, r.start)..byte_for_utf16(&text, r.end));
        let Some(mark) = &self.composition else {
            return (
                requested.unwrap_or_else(|| self.selected_bytes()),
                new.into(),
                0,
            );
        };
        let marked = mark.target.start..mark.target.start + mark.text.len();
        let r = requested.unwrap_or(marked.clone());
        let start = r.start.min(marked.start);
        let end = r.end.max(marked.end);
        let mut replacement = text[start..r.start].to_owned();
        let prefix = replacement.len();
        replacement.push_str(new);
        replacement.push_str(&text[r.end..end]);
        (
            start..end - marked.len() + mark.target.len(),
            replacement,
            prefix,
        )
    }

    fn commit(&mut self, range: Range<usize>, text: String) {
        let target = if range == self.selected_bytes() {
            self.engine.selection()
        } else {
            Selection {
                anchor: self.position_at_byte(range.start),
                focus: self.position_at_byte(range.end),
            }
        };
        self.edit(Command::ReplaceRange { target, text });
    }

    pub(super) fn finish_composition(&mut self) {
        if let Some(mark) = self.composition.take() {
            self.commit(mark.target, mark.text);
        }
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        actual: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.input_text();
        let range = byte_for_utf16(&text, range.start)..byte_for_utf16(&text, range.end);
        *actual = Some(
            text[..range.start].encode_utf16().count()..text[..range.end].encode_utf16().count(),
        );
        Some(text[range].into())
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.input_text();
        let (bytes, reversed) = if let Some(m) = &self.composition {
            (
                m.target.start + m.selected.start..m.target.start + m.selected.end,
                false,
            )
        } else {
            let sel = self.engine.selection();
            (
                self.selected_bytes(),
                self.global_byte(sel.anchor) > self.global_byte(sel.focus),
            )
        };
        Some(UTF16Selection {
            range: text[..bytes.start].encode_utf16().count()
                ..text[..bytes.end].encode_utf16().count(),
            reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let mark = self.composition.as_ref()?;
        let start = self.plain_text()[..mark.target.start]
            .encode_utf16()
            .count();
        Some(start..start + mark.text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.finish_composition();
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (target, text, _) = self.replacement(range, new);
        self.commit(target, text);
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (target, text, prefix) = self.replacement(range, new);
        let selected = selected
            .map(|r| prefix + byte_for_utf16(new, r.start)..prefix + byte_for_utf16(new, r.end))
            .unwrap_or(prefix + new.len()..prefix + new.len());
        self.composition = Some(Composition {
            target,
            text,
            selected,
        });
        self.reveal = true;
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let text = self.input_text();
        let byte = byte_for_utf16(&text, range.start);
        let (pos, delta) = if let Some(m) = &self.composition
            && byte >= m.target.start
        {
            let end = m.target.start + m.text.len();
            if byte <= end {
                (self.position_at_byte(m.target.start), byte - m.target.start)
            } else {
                (self.position_at_byte(byte - end + m.target.end), 0)
            }
        } else {
            (self.position_at_byte(byte), 0)
        };
        let row = &self.rows[self.row_index(pos)];
        let layout = row.layout.as_ref()?;
        Some(Bounds::new(
            layout.position_for_index(row.boundaries[pos.offset] + delta)?,
            size(px(1.), layout.line_height()),
        ))
    }

    fn character_index_for_point(
        &mut self,
        p: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let byte = self.global_byte(self.hit(p)?);
        Some(self.plain_text()[..byte].encode_utf16().count())
    }
}
