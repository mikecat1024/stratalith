//! A cursor in indentation columns. Consuming part of a tab leaves virtual
//! spaces; unconsumed tabs in literal code or HTML keep their spelling.

#[derive(Clone, Copy)]
pub(super) struct Line<'a> {
    rest: &'a str,
    column: usize,
    spaces: usize,
}

impl<'a> Line<'a> {
    pub fn new(rest: &'a str) -> Self {
        Self {
            rest,
            column: 0,
            spaces: 0,
        }
    }
    pub fn content(self) -> &'a str {
        self.rest.trim_start_matches([' ', '\t'])
    }
    pub fn blank(self) -> bool {
        self.content().is_empty()
    }

    pub fn indent(self) -> usize {
        let mut col = self.column + self.spaces;
        for b in self.rest.bytes() {
            match b {
                b' ' => col += 1,
                b'\t' => col += 4 - col % 4,
                _ => break,
            }
        }
        col - self.column
    }

    pub fn skip(mut self, mut n: usize) -> Self {
        let virtual_spaces = n.min(self.spaces);
        self.spaces -= virtual_spaces;
        self.column += virtual_spaces;
        n -= virtual_spaces;
        while n > 0 {
            let width = if self.rest.starts_with('\t') {
                4 - self.column % 4
            } else {
                1
            };
            self.rest = &self.rest[1..];
            let used = n.min(width);
            self.column += used;
            self.spaces = width - used;
            n -= used;
        }
        self
    }

    pub fn write(self, output: &mut String) {
        output.extend(std::iter::repeat_n(' ', self.spaces));
        output.push_str(self.rest);
        output.push('\n');
    }
}
