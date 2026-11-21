mod images;
mod input;
mod text;

use gpui::prelude::*;
use gpui::*;
use std::ops::Range;
use std::time::Instant;
use stratalith_core::builder::{document, paragraph};
use stratalith_core::{
    Affinity, BlockKind, BlockPosition, Command, EditorEngine, ListKind, NodeId, NodeKind,
    Selection,
};
use stratalith_markdown::{paste_command, serialize_fragment};
use text::BlockText;

const PLACEHOLDER: &str =
    "Start typing. Type # followed by a space for a heading, or **text** for bold.";
const BODY_SIZE: f32 = 16.;
const LINE_HEIGHT: f32 = 1.65;

struct Editor {
    engine: EditorEngine,
    focus: FocusHandle,
    rows: Vec<BlockText>,
    started: Instant,
    message: Option<String>,
    composition: Option<input::Composition>,
    scroll: ScrollHandle,
    reveal: bool,
}

impl Editor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let doc = document([paragraph([])]).unwrap();
        let at = BlockPosition {
            block: doc.blocks()[0],
            offset: 0,
            affinity: Affinity::Downstream,
        };
        let focus = cx.focus_handle();
        window.focus(&focus);
        let rows = vec![BlockText::new(&doc, at.block).unwrap()];
        Self {
            engine: EditorEngine::new(doc, Selection::caret(at)).unwrap(),
            focus,
            rows,
            started: Instant::now(),
            message: None,
            composition: None,
            scroll: ScrollHandle::new(),
            reveal: false,
        }
    }

    fn edit(&mut self, command: Command) {
        self.composition = None;
        match self.engine.execute(
            command,
            self.engine.selection(),
            Some(self.started.elapsed()),
        ) {
            Ok(changed) => {
                self.message = None;
                if changed {
                    let doc = self.engine.document();
                    let mut start = 0;
                    self.rows = doc
                        .nodes()
                        .filter_map(|n| BlockText::new(doc, n.id()))
                        .map(|mut row| {
                            row.start = start;
                            start += row.text.len() + 1;
                            row
                        })
                        .collect();
                    self.reveal = true;
                }
            }
            Err(e) => self.message = Some(e.to_string()),
        }
    }

    fn select(&mut self, pos: BlockPosition, extend: bool) {
        let anchor = if extend {
            self.engine.selection().anchor
        } else {
            pos
        };
        if let Err(e) = self.engine.set_selection(Selection { anchor, focus: pos }) {
            self.message = Some(e.to_string());
        }
    }

    fn row_index(&self, pos: BlockPosition) -> usize {
        self.rows.iter().position(|r| r.id == pos.block).unwrap()
    }

    fn hit(&self, point: Point<Pixels>) -> Option<BlockPosition> {
        let mut nearest = None;
        for row in &self.rows {
            let layout = row.layout.as_ref()?;
            let byte = layout.index_for_position(point).unwrap_or_else(|i| i);
            let bounds = layout.bounds();
            let distance = (bounds.top() - point.y)
                .max(point.y - bounds.bottom())
                .max(px(0.));
            if nearest.is_none_or(|(d, _)| distance < d) {
                nearest = Some((distance, row.position(byte)));
            }
        }
        nearest.map(|(_, p)| p)
    }

    fn navigate(&mut self, key: &str, shift: bool, edge: bool, word: bool) {
        let selection = self.engine.selection();
        let pos = selection.focus;
        let index = self.row_index(pos);
        let row = &self.rows[index];
        let byte = row.boundaries[pos.offset];
        let next = match key {
            "left" | "right" if !shift && selection.anchor != pos => {
                let range = selection.range(self.engine.document()).unwrap();
                if key == "left" {
                    range.start()
                } else {
                    range.end()
                }
            }
            "home" => row.position(0),
            "end" => row.position(row.text.len()),
            "left" if edge => row.position(0),
            "right" if edge => row.position(row.text.len()),
            "left" if word && byte > 0 => row.word_edge(byte, true),
            "right" if word && byte < row.text.len() => row.word_edge(byte, false),
            "left" if pos.offset > 0 => row.position(row.boundaries[pos.offset - 1]),
            "right" if pos.offset + 1 < row.boundaries.len() => {
                row.position(row.boundaries[pos.offset + 1])
            }
            "left" if index > 0 => self.rows[index - 1].position(usize::MAX),
            "right" if index + 1 < self.rows.len() => self.rows[index + 1].position(0),
            "up" | "down" => {
                if edge {
                    if key == "up" {
                        self.rows[0].position(0)
                    } else {
                        self.rows.last().unwrap().position(usize::MAX)
                    }
                } else {
                    let Some(layout) = &row.layout else { return };
                    let Some(mut p) = layout.position_for_index(byte) else {
                        return;
                    };
                    p.y += if key == "up" {
                        -layout.line_height() * 0.5
                    } else {
                        layout.line_height() * 1.5
                    };
                    if p.y < layout.bounds().top() && index > 0 {
                        if let Some(previous) = &self.rows[index - 1].layout {
                            p.y = previous.bounds().bottom() - px(1.);
                        }
                    } else if p.y >= layout.bounds().bottom()
                        && index + 1 < self.rows.len()
                        && let Some(next) = &self.rows[index + 1].layout
                    {
                        p.y = next.bounds().top() + px(1.);
                    }
                    self.hit(p).unwrap_or(pos)
                }
            }
            _ => pos,
        };
        self.select(next, shift);
        self.reveal = true;
    }

    fn key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.composition.is_some() {
            return;
        }
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        let shortcut = mods.platform || mods.control;
        let word = if cfg!(target_os = "macos") {
            mods.alt
        } else {
            mods.control
        };
        let sel = self.engine.selection();
        match (key, shortcut) {
            ("z", true) if !mods.shift => self.edit(Command::Undo),
            ("z" | "y", true) => self.edit(Command::Redo),
            ("a", true) => {
                self.select(self.rows[0].position(0), false);
                self.select(self.rows.last().unwrap().position(usize::MAX), true);
            }
            ("c" | "x", true) if sel.anchor != sel.focus => {
                match self
                    .engine
                    .document()
                    .extract(sel)
                    .map_err(|e| e.to_string())
                    .and_then(|doc| serialize_fragment(&doc).map_err(|e| e.to_string()))
                {
                    Ok(text) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(text));
                        if key == "x" {
                            self.edit(Command::ReplaceRange {
                                target: sel,
                                text: String::new(),
                            });
                        }
                    }
                    Err(e) => self.message = Some(e),
                }
            }
            ("v", true) => {
                if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    match paste_command(self.engine.document(), sel, &text) {
                        Ok(Some(command)) => self.edit(command),
                        Ok(None) => (),
                        Err(e) => self.message = Some(e.to_string()),
                    }
                }
            }
            ("left" | "right" | "up" | "down" | "home" | "end", _) => {
                self.navigate(key, mods.shift, mods.platform, word)
            }
            ("backspace" | "delete", _)
                if (word || mods.platform) && self.selected_bytes().is_empty() =>
            {
                let row = &self.rows[self.row_index(sel.focus)];
                let backward = key == "backspace";
                let byte = row.boundaries[sel.focus.offset];
                let focus = if mods.platform {
                    row.position(if backward { 0 } else { row.text.len() })
                } else {
                    row.word_edge(byte, backward)
                };
                if focus.offset == sel.focus.offset {
                    self.edit(if backward {
                        Command::Backspace
                    } else {
                        Command::Delete
                    });
                } else {
                    self.edit(Command::ReplaceRange {
                        target: Selection {
                            anchor: sel.focus,
                            focus,
                        },
                        text: String::new(),
                    });
                }
            }
            ("backspace", false) => self.edit(Command::Backspace),
            ("delete", false) => self.edit(Command::Delete),
            ("enter", false) if mods.shift => self.edit(Command::InsertHardBreak),
            ("enter", false) => self.edit(Command::Enter),
            ("tab", false) if mods.shift => self.edit(Command::OutdentListItem),
            ("tab", false) => self.edit(Command::IndentListItem),
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }
}

impl Editor {
    fn text_element(
        &mut self,
        index: usize,
        selection: &Range<usize>,
        placeholder: bool,
        foreground: Hsla,
        window: &Window,
    ) -> Div {
        let start = self.rows[index].start;
        let focus = self.engine.selection().focus;
        let row = &mut self.rows[index];
        let len = row.text.len();
        let local = |byte: usize| byte.saturating_sub(start).min(len);
        let selected = local(selection.start)..local(selection.end);
        let mut highlights = Vec::new();
        for (range, style) in &row.styles {
            for (slice, active) in [
                (range.start..range.end.min(selected.start), false),
                (
                    range.start.max(selected.start)..range.end.min(selected.end),
                    true,
                ),
                (range.start.max(selected.end)..range.end, false),
            ] {
                if !slice.is_empty() {
                    let mut style = *style;
                    if active {
                        style.background_color = Some(rgba(0x4285f477).into());
                    }
                    highlights.push((slice, style));
                }
            }
        }
        let mut text = row.text.clone();
        let mut caret =
            (row.id == focus.block && selection.is_empty()).then(|| row.boundaries[focus.offset]);
        if let Some(mark) = &self.composition {
            let replaced = local(mark.target.start)..local(mark.target.end);
            let first = (start..=start + len).contains(&mark.target.start);
            if first || !replaced.is_empty() {
                let insert = if first { mark.text.as_str() } else { "" };
                text.replace_range(replaced.clone(), insert);
                highlights.clear();
                if first {
                    highlights.push((
                        replaced.start..replaced.start + insert.len(),
                        HighlightStyle {
                            underline: Some(UnderlineStyle {
                                thickness: px(1.),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ));
                }
                caret = first.then_some(replaced.start + mark.selected.end);
            }
        } else if placeholder {
            text = PLACEHOLDER.into();
            highlights = vec![(
                0..text.len(),
                HighlightStyle {
                    color: Some(rgb(0x808080).into()),
                    ..Default::default()
                },
            )];
        }
        if text.is_empty() || text.ends_with('\n') {
            text.push('\u{200b}');
        }
        let styled = StyledText::new(text).with_highlights(highlights);
        let layout = styled.layout().clone();
        row.layout = Some(layout.clone());
        let mut element = div().relative().w_full().child(styled);
        if let Some(caret) = caret
            && self.focus.is_focused(window)
        {
            let scroll = self.scroll.clone();
            let reveal = self.reveal;
            element = element.child(
                canvas(
                    |_, _, _| (),
                    move |_, _, window, _| {
                        if let Some(origin) = layout.position_for_index(caret) {
                            let h = layout.line_height();
                            window.paint_quad(fill(
                                Bounds::new(origin, size(px(1.5), h)),
                                foreground,
                            ));
                            if reveal {
                                let bottom = window.viewport_size().height;
                                let delta = if origin.y < px(0.) {
                                    -origin.y
                                } else {
                                    (bottom - origin.y - h).min(px(0.))
                                };
                                if delta != px(0.) {
                                    let mut offset = scroll.offset();
                                    offset.y += delta;
                                    scroll.set_offset(offset);
                                    window.refresh();
                                }
                            }
                        }
                    },
                )
                .absolute()
                .inset_0(),
            );
        }
        element
    }

    fn block(
        &mut self,
        id: NodeId,
        list_depth: usize,
        next: &mut usize,
        selection: &Range<usize>,
        foreground: Hsla,
        window: &Window,
    ) -> Div {
        let doc = self.engine.document();
        let node = doc.node(id).unwrap();
        let NodeKind::Block(kind) = node.kind() else {
            unreachable!()
        };
        let panel = rgba(0x88888822);
        match kind {
            BlockKind::Paragraph | BlockKind::Heading { .. } | BlockKind::CodeBlock { .. } => {
                let placeholder = matches!(kind, BlockKind::Paragraph)
                    && doc.blocks() == [id]
                    && self.rows[*next].text.is_empty();
                let mut frame = div().w_full();
                match kind {
                    BlockKind::Heading { level } => {
                        let font_size = [32., 24., 18.72, 16., 13.28, 10.72][*level as usize];
                        frame = frame
                            .text_size(px(font_size))
                            .line_height(px(font_size * LINE_HEIGHT))
                            .when(*level as usize == 0, |s| s.mb(px(32. * 0.67 - 16.)))
                            .font_weight(FontWeight::BOLD);
                    }
                    BlockKind::CodeBlock { .. } => {
                        frame = frame.p_3().rounded_sm().bg(panel).font_family("monospace")
                    }
                    _ => (),
                }
                let text = self.text_element(*next, selection, placeholder, foreground, window);
                frame = frame.child(text);
                for url in &self.rows[*next].images {
                    frame = frame.child(img(url.clone()).max_w_full());
                }
                *next += 1;
                frame
            }
            BlockKind::ThematicBreak => div().w_full().h_px().bg(rgb(0x808080)),
            BlockKind::Html(value) => div()
                .p_3()
                .bg(panel)
                .font_family("monospace")
                .child(value.clone()),
            _ => {
                let mut frame = div().flex().flex_col().w_full().gap_4();
                let mut list = None;
                match kind {
                    BlockKind::BlockQuote => {
                        frame = frame.border_l_3().border_color(rgb(0x808080)).pl_4()
                    }
                    BlockKind::List { kind, tight } => {
                        list = Some(*kind);
                        frame = frame.when(*tight, |s| s.gap_0());
                    }
                    BlockKind::ListItem => {
                        let parent = node.parent().and_then(|id| doc.node(id));
                        if matches!(
                            parent.map(|n| n.kind()),
                            Some(NodeKind::Block(BlockKind::List { tight: true, .. }))
                        ) {
                            frame = frame.gap_0();
                        }
                    }
                    _ => (),
                }
                let children = node.children().to_vec();
                for (i, child) in children.into_iter().enumerate() {
                    let body = self.block(
                        child,
                        list_depth + usize::from(list.is_some()),
                        next,
                        selection,
                        foreground,
                        window,
                    );
                    frame = frame.child(if let Some(kind) = list {
                        let marker = div()
                            .flex()
                            .justify_end()
                            .items_center()
                            .min_w(px(40.))
                            .h(px(BODY_SIZE * LINE_HEIGHT));
                        let marker = match kind {
                            ListKind::Bullet => {
                                let shape = div().size(px(5.));
                                let shape = match list_depth {
                                    0 => shape.rounded_full().bg(foreground),
                                    1 => shape.rounded_full().border_1().border_color(foreground),
                                    _ => shape.bg(foreground),
                                };
                                marker.pr(px(12.)).child(shape)
                            }
                            ListKind::Ordered { start } => {
                                marker.pr(px(5.)).child(format!("{}.", start as usize + i))
                            }
                        };
                        div()
                            .flex()
                            .child(marker)
                            .child(div().flex_1().min_w_0().child(body))
                    } else {
                        body
                    });
                }
                frame
            }
        }
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let foreground: Hsla = if dark { white() } else { black() };
        let padding = (window.viewport_size().height * 0.06).clamp(px(24.), px(64.));
        let mut column = div()
            .w_full()
            .max_w(px(672.))
            .mx_auto()
            .flex()
            .flex_col()
            .gap_4()
            .py(padding)
            .children(self.message.clone());
        let mut next = 0;
        let selection = self.selected_bytes();
        for id in self.engine.document().blocks().to_vec() {
            column = column.child(self.block(id, 0, &mut next, &selection, foreground, window));
        }
        self.reveal = false;
        let entity = cx.entity();
        let focus = self.focus.clone();
        div()
            .id("editor")
            .size_full()
            .relative()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .px_5()
            .bg(if dark { black() } else { white() })
            .text_color(foreground)
            .text_size(px(BODY_SIZE))
            .line_height(px(BODY_SIZE * LINE_HEIGHT))
            .cursor_text()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, window, cx| {
                    window.focus(&view.focus);
                    let pos = view.hit(event.position);
                    view.finish_composition();
                    if let Some(pos) = pos
                        && pos.to_node_position(view.engine.document()).is_ok()
                    {
                        let row = &view.rows[view.row_index(pos)];
                        if (event.modifiers.platform || event.modifiers.control)
                            && let Some((_, url)) = row
                                .links
                                .iter()
                                .find(|(range, _)| range.contains(&row.boundaries[pos.offset]))
                        {
                            cx.open_url(url);
                        } else if event.click_count >= 3 {
                            let end = row.position(usize::MAX);
                            view.select(row.position(0), false);
                            view.select(end, true);
                        } else if event.click_count == 2 {
                            let byte = row.boundaries[pos.offset];
                            let left = row.text[..byte]
                                .trim_end_matches(char::is_alphanumeric)
                                .len();
                            let right = row.text.len()
                                - row.text[byte..]
                                    .trim_start_matches(char::is_alphanumeric)
                                    .len();
                            let end = row.position(right);
                            view.select(row.position(left), false);
                            view.select(end, true);
                        } else {
                            view.select(pos, event.modifiers.shift);
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _, cx| {
                if event.dragging()
                    && let Some(pos) = view.hit(event.position)
                {
                    view.select(pos, true);
                    cx.notify();
                }
            }))
            .child(column)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        window.handle_input(&focus, ElementInputHandler::new(bounds, entity), cx);
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

fn main() {
    let app = Application::new();
    let client = images::ImageClient(app.background_executor());
    app.with_http_client(std::sync::Arc::new(client)).run(|cx| {
        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.open_window(WindowOptions::default(), |window, cx| {
            window.set_window_title("Stratalith Editor");
            cx.new(|cx| Editor::new(window, cx))
        })
        .expect("open the Markdown editor window");
        cx.activate(true);
    });
}
