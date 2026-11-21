use super::{Inline, InlineSequence, Run};
use crate::{MarkdownError, error::check_depth};

pub(super) enum Piece {
    Node(Inline),
    Run(Run),
}

struct Opener {
    run: Run,
    // Older openers cannot change while this one is on the stack.
    outer_depth: usize,
    remaining: usize,
    before: InlineSequence,
}

impl Opener {
    fn inner_depth(&self) -> usize {
        self.outer_depth + self.remaining
    }

    fn matches(&self, closer: &Run) -> bool {
        // CommonMark's rule of three disambiguates adjacent intraword runs.
        self.run.mark == closer.mark
            && (!(self.run.closes || closer.opens)
                || !(self.run.width + closer.width).is_multiple_of(3)
                || self.run.width.is_multiple_of(3))
    }

    fn literal(mut self, children: InlineSequence) -> InlineSequence {
        self.before.push(Inline::Text(
            self.run.mark.to_string().repeat(self.remaining),
        ));
        for child in children.into_nodes() {
            self.before.push(child);
        }
        self.before
    }
}

// Brackets bind before emphasis. Each completed link resolves only its label;
// failed brackets remain literal and do not split the surrounding delimiter run.
pub(super) fn resolve(
    pieces: impl IntoIterator<Item = Piece>,
    depth: usize,
) -> Result<InlineSequence, MarkdownError> {
    let mut nodes = InlineSequence::default();
    let mut open: Vec<Opener> = Vec::new();
    for piece in pieces {
        let run = match piece {
            Piece::Run(run) => run,
            Piece::Node(node) => {
                nodes.push(node);
                continue;
            }
        };
        let mut width = run.width;
        while run.closes && width > 0 {
            let Some(index) = open.iter().rposition(|o| o.matches(&run)) else {
                break;
            };
            while open.len() > index + 1 {
                nodes = open.pop().unwrap().literal(nodes);
            }
            let opener = open.last_mut().unwrap();
            let n = width.min(opener.remaining).min(2);
            width -= n;
            opener.remaining -= n;
            let children = std::mem::take(&mut nodes);
            if opener.remaining == 0 {
                nodes = open.pop().unwrap().before;
            }
            nodes.push(if n == 2 {
                Inline::Strong(children)
            } else {
                Inline::Emphasis(children)
            });
        }
        if width > 0 && run.opens {
            let outer_depth = open.last().map_or(depth, Opener::inner_depth);
            check_depth(outer_depth + width)?;
            open.push(Opener {
                run,
                outer_depth,
                remaining: width,
                before: std::mem::take(&mut nodes),
            });
        } else if width > 0 {
            nodes.push(Inline::Text(run.mark.to_string().repeat(width)));
        }
    }
    while let Some(frame) = open.pop() {
        nodes = frame.literal(nodes);
    }
    check_depth(depth + nodes.height())?;
    Ok(nodes)
}
