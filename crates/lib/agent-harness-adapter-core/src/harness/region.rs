//! The `region` kind: the tool owns one block between its markers in the
//! user's file.

use crate::{eol::normalise, harness::meaning, integration::BlockOrder};

/// A tool's region markers: `<!-- <tool>:harness -->` and
/// `<!-- /<tool>:harness -->`, each on a line of its own; the opening one
/// is `<!-- <tool>:harness last -->` for a tool whose block comes last
/// ([`BlockOrder::Last`]). Either form opens the tool's region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markers {
    tool: String,
    order: BlockOrder,
    /// The opening marker this tool writes, built by [`Markers::order`].
    open: String,
    close: String,
}

/// One tool's block among the lines: its opening and closing lines, and
/// whether it comes last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Block {
    open: usize,
    close: usize,
    last: bool,
}

/// The tool's name and whether it asks to come last, when `line` (its end
/// trimmed) is any tool's opening marker.
fn opening(line: &str) -> Option<(&str, bool)> {
    let inner = line.strip_prefix("<!-- ")?.strip_suffix(" -->")?;
    let (name, last) = match inner.strip_suffix(":harness last") {
        Some(name) => (name, true),
        None => (inner.strip_suffix(":harness")?, false),
    };
    (!name.is_empty() && !name.starts_with('/')).then_some((name, last))
}

/// The tool's name when `line` (its end trimmed) is any tool's closing
/// marker.
fn closing(line: &str) -> Option<&str> {
    let name = line.strip_prefix("<!-- /")?.strip_suffix(":harness -->")?;
    (!name.is_empty()).then_some(name)
}

/// A code fence's character and length, when `line` opens or closes one:
/// up to three spaces, then three or more backticks or tildes.
fn fence(line: &str) -> Option<(char, usize)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let c = t.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = t.chars().take_while(|x| *x == c).count();
    // A backtick fence's info string holds no backtick: such a line is
    // inline code.
    let info = &t[n..];
    (n >= 3 && !(c == '`' && info.contains('`'))).then_some((c, n))
}

/// Whether each of `lines` is fenced code, its fences included, by the
/// rules of [`unfenced`].
fn fenced(lines: &[&str]) -> Vec<bool> {
    let mut open: Option<(char, usize)> = None;
    lines
        .iter()
        .map(|l| match (open, fence(l)) {
            (None, Some(f)) => {
                open = Some(f);
                true
            }
            (Some((c, n)), Some((d, m)))
                if c == d && m >= n && l.trim().chars().all(|x| x == c) =>
            {
                open = None;
                true
            }
            (None, None) => false,
            _ => true,
        })
        .collect()
}

/// `lines` with each line of fenced code made empty, fences included, so a
/// marker shown in an example is no block. A fence closes at a line of the
/// same character, at least as long, with nothing after it; an unclosed
/// fence runs to the end, as in `CommonMark`.
fn unfenced<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    lines
        .iter()
        .zip(fenced(lines))
        .map(|(l, f)| if f { "" } else { *l })
        .collect()
}

impl Markers {
    /// The markers of the tool named `tool`, its block in the order it was
    /// first written ([`BlockOrder::Installed`]).
    pub fn new(tool: &str) -> Self {
        Markers {
            tool: tool.to_string(),
            order: BlockOrder::Installed,
            open: format!("<!-- {tool}:harness -->"),
            close: format!("<!-- /{tool}:harness -->"),
        }
    }

    /// The markers of a tool whose block stands in the order `order` among
    /// other tools' blocks (KIT-26).
    // sokf-implements: KIT-26_AC-1
    #[must_use]
    pub fn order(mut self, order: BlockOrder) -> Self {
        let tool = &self.tool;
        self.open = match order {
            BlockOrder::Last => format!("<!-- {tool}:harness last -->"),
            BlockOrder::Installed => format!("<!-- {tool}:harness -->"),
        };
        self.order = order;
        self
    }

    /// The opening marker this tool writes.
    pub fn open(&self) -> &str {
        &self.open
    }

    /// The closing marker.
    pub fn close(&self) -> &str {
        &self.close
    }

    /// The line indexes of the markers in LF text: the first closing line
    /// that has an opening line (in either form) before it, and the nearest
    /// such opening line. A stray opening marker (its closing one deleted) is
    /// then just the user's text: the region never swallows what lies between
    /// it and a later region, so `--force` cannot delete it.
    // sokf-implements: KIT-9_AC-1
    // sokf-implements: KIT-26_AC-1
    fn locate(&self, lines: &[&str]) -> Option<(usize, usize)> {
        let opens = |l: &str| opening(l).is_some_and(|(name, _)| name == self.tool);
        lines
            .iter()
            .enumerate()
            .filter(|(_, l)| l.trim_end() == self.close)
            .find_map(|(c, _)| {
                let o = lines[..c].iter().rposition(|l| opens(l.trim_end()))?;
                Some((o, c))
            })
    }

    /// The blocks in `lines`, in file order: the tool's region (with the
    /// tool's choice), then each other tool's block, located by the rule of
    /// [`locate`](Markers::locate) with its markers outside fenced code, that
    /// meets no block kept before it; a block inside another is its text.
    fn blocks(&self, lines: &[&str]) -> Vec<Block> {
        let own = self.locate(lines).map(|(open, close)| Block {
            open,
            close,
            last: self.order == BlockOrder::Last,
        });
        let plain = unfenced(lines);
        let mut names: Vec<&str> = Vec::new();
        for (name, _) in plain.iter().filter_map(|l| opening(l.trim_end())) {
            if name != self.tool && !names.contains(&name) {
                names.push(name);
            }
        }
        let mut others: Vec<Block> = names
            .into_iter()
            .filter_map(|name| {
                let (open, close) = Markers::new(name).locate(&plain)?;
                let last = opening(plain[open].trim_end()).is_some_and(|(_, last)| last);
                Some(Block { open, close, last })
            })
            .collect();
        others.sort_by_key(|b| b.open);
        // A region that meets another tool's block (one inside the other,
        // or crossing) leaves the order alone: moving either would change
        // the other's content.
        let meets = |b: &Block, k: &Block| !(b.close < k.open || b.open > k.close);
        if own.is_some_and(|own| others.iter().any(|b| meets(b, &own))) {
            return Vec::new();
        }
        let mut all: Vec<Block> = own.into_iter().collect();
        for b in others {
            if !all.iter().any(|k| meets(&b, k)) {
                all.push(b);
            }
        }
        all.sort_by_key(|b| b.open);
        all
    }

    /// Whether some block that does not come last follows one that does.
    fn unordered(&self, lines: &[&str]) -> bool {
        let blocks = self.blocks(lines);
        blocks.iter().skip_while(|b| !b.last).any(|b| !b.last)
    }

    /// `lines` with their blocks in order: every block that does not come
    /// last before every block that does, each kind keeping its order. The
    /// blocks swap places with their lines as found; the lines between them
    /// stay where they are.
    // sokf-implements: KIT-26_AC-2
    fn arrange<'a>(&self, lines: &[&'a str]) -> Vec<&'a str> {
        let blocks = self.blocks(lines);
        let mut order: Vec<Block> = blocks.clone();
        order.sort_by_key(|b| b.last);
        if order == blocks {
            return lines.to_vec();
        }
        let mut out: Vec<&str> = lines[..blocks[0].open].to_vec();
        for (k, slot) in blocks.iter().enumerate() {
            let b = order[k];
            out.extend(&lines[b.open..=b.close]);
            let end = blocks.get(k + 1).map_or(lines.len(), |next| next.open);
            out.extend(&lines[slot.close + 1..end]);
        }
        out
    }

    /// The block with its markers, LF; a blank line after the opening
    /// marker when the block is not empty, so the file reads as Markdown.
    fn framed(&self, block: &str) -> String {
        let body = Markers::body(block);
        let gap = if body.is_empty() { "" } else { "\n" };
        format!("{}\n{gap}{body}{}\n", self.open, self.close)
    }

    /// `block` as the region holds it and [`find`](Markers::find) reads it
    /// back: LF, with a final newline when it isn't empty.
    pub(crate) fn body(block: &str) -> String {
        let mut body = normalise(block);
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        body
    }

    /// The block between the markers in `text`, LF, with its trailing
    /// newline; `None` when the markers are absent. The one blank line after
    /// the opening marker belongs to the markers, not the block, so a block
    /// reads back exactly as [`render`](Markers::render) wrote it.
    // sokf-implements: KIT-9_AC-4
    pub fn find(&self, text: &str) -> Option<String> {
        let text = normalise(text);
        let lines: Vec<&str> = text.split('\n').collect();
        let (open, close) = self.locate(&lines)?;
        let mut inner = &lines[open + 1..close];
        if inner.first().is_some_and(|l| l.trim().is_empty()) {
            inner = &inner[1..];
        }
        if inner.is_empty() {
            return Some(String::new());
        }
        Some(format!("{}\n", inner.join("\n")))
    }

    /// The file to write: `block` rewritten between the markers, appended
    /// after one blank line when they are absent, or alone for an absent or
    /// empty file; then the blocks put in their order (KIT-26). The file's
    /// line endings are kept, and nothing outside the region changes but
    /// the place of the blocks. `None` when nothing changes.
    // sokf-implements: KIT-9_AC-2
    // sokf-implements: KIT-9_AC-3
    pub fn render(&self, existing: Option<&str>, block: &str) -> Option<String> {
        let new_block = self.framed(block);
        let Some(existing) = existing else {
            return Some(new_block);
        };
        let crlf = existing.contains("\r\n");
        let text = normalise(existing);
        let lines: Vec<&str> = text.split('\n').collect();
        let out = self.put(&lines, self.locate(&lines), &new_block);
        let out = if crlf { out.replace('\n', "\r\n") } else { out };
        (out != existing).then_some(out)
    }

    /// `existing` with the blocks in their order and the region opened by
    /// the marker this tool writes, its lines otherwise as found; `None`
    /// when both hold already, or there is no region. The file's line
    /// endings are kept. The block between the markers stays as found, so
    /// its state and its hash don't change.
    // sokf-implements: KIT-26_AC-4
    // sokf-implements: KIT-26_AC-5
    pub(crate) fn place(&self, existing: &str) -> Option<String> {
        let crlf = existing.contains("\r\n");
        // Mixed line endings: a move would rewrite them all, so the region
        // is left where it stands.
        if crlf && existing.replace("\r\n", "").contains('\n') {
            return None;
        }
        let text = normalise(existing);
        let lines: Vec<&str> = text.split('\n').collect();
        let (open, close) = self.locate(&lines)?;
        if lines[open].trim_end() == self.open && !self.unordered(&lines) {
            return None;
        }
        let mut region = vec![self.open.as_str()];
        region.extend(&lines[open + 1..=close]);
        let region = format!("{}\n", region.join("\n"));
        let out = self.put(&lines, Some((open, close)), &region);
        let out = if crlf { out.replace('\n', "\r\n") } else { out };
        (out != existing).then_some(out)
    }

    /// Whether the blocks of `text` are out of their order (KIT-26).
    pub(crate) fn out_of_order(&self, text: &str) -> bool {
        let text = normalise(text);
        self.unordered(&text.split('\n').collect::<Vec<_>>())
    }

    /// LF `lines` with `region` (framed, LF) where the tool's region `own`
    /// stands, else added ([`insert`](Markers::insert)), then the blocks in
    /// their order.
    fn put(&self, lines: &[&str], own: Option<(usize, usize)>, region: &str) -> String {
        let text = match own {
            Some((open, close)) => {
                let head = lines[..open].join("\n");
                let tail = lines[close + 1..].join("\n");
                let head = if open == 0 { head } else { format!("{head}\n") };
                format!("{head}{region}{tail}")
            }
            None => self.insert(&lines.join("\n"), region),
        };
        let lines: Vec<&str> = text.split('\n').collect();
        self.arrange(&lines).join("\n")
    }

    /// LF `text`, which holds no region of the tool, with `region` added:
    /// alone in an empty file; for a tool that is not last, before the
    /// first block that comes last, with one blank line after it; else
    /// appended after one blank line.
    // sokf-implements: KIT-26_AC-3
    fn insert(&self, text: &str, region: &str) -> String {
        if text.is_empty() {
            return region.to_string();
        }
        if self.order != BlockOrder::Last {
            let lines: Vec<&str> = text.split('\n').collect();
            if let Some(at) = self.blocks(&lines).iter().find(|b| b.last).map(|b| b.open) {
                let mut out: Vec<&str> = lines[..at].to_vec();
                out.extend(region.trim_end_matches('\n').split('\n'));
                out.push("");
                out.extend(&lines[at..]);
                return out.join("\n");
            }
        }
        let mut head = text.to_string();
        if !head.ends_with('\n') {
            head.push('\n');
        }
        format!("{head}\n{region}")
    }

    /// The file with `text` kept as plain text right after the region, or
    /// at its end when it holds none, or alone for an absent or empty file;
    /// `None` when the text already stands there or is only whitespace.
    // sokf-implements: KIT-27_AC-3
    // sokf-implements: KIT-27_AC-4
    pub(crate) fn keep(&self, name: &str, existing: Option<&str>, text: &str) -> Option<String> {
        let text = normalise(text);
        let kept: Vec<&str> = text
            .trim_end()
            .split('\n')
            .skip_while(|l| l.trim().is_empty())
            .collect();
        if kept.iter().all(|l| l.trim().is_empty()) {
            return None;
        }
        let Some(existing) = existing.filter(|e| !e.is_empty()) else {
            return Some(format!("{}\n", kept.join("\n")));
        };
        // Each line as found, with its own ending, so a file of mixed line
        // endings keeps them; `lines` without the `\r`.
        let raw: Vec<&str> = existing.split('\n').collect();
        let lines: Vec<&str> = raw
            .iter()
            .map(|l| l.strip_suffix('\r').unwrap_or(l))
            .collect();
        let region = self.locate(&lines);
        let blank = |i: usize| lines.get(i).is_none_or(|l| l.trim().is_empty());
        // Already held: the text's lines in a row, outside every block and
        // fenced code, so a retry writes nothing whatever was kept, moved or
        // reflowed around it since.
        let plain = unfenced(&lines);
        let blocks = self.blocks(&lines);
        let outside = |k: usize| {
            region.is_none_or(|(o, c)| k < o || k > c)
                && !blocks.iter().any(|b| b.open <= k && k <= b.close)
                && (plain[k] == lines[k] || lines[k].is_empty())
        };
        let n = kept.len();
        let held = lines.len() >= n
            && (0..=lines.len() - n).any(|i| {
                (i..i + n).all(outside)
                    && lines[i..i + n]
                        .iter()
                        .zip(&kept)
                        .all(|(a, b)| a.trim_end() == b.trim_end())
            });
        if held || Markers::held_by_meaning(name, &lines, region, &blocks, &kept) {
            return None;
        }
        // The ending of line `i`: its own, or the file's for a last line
        // without one.
        let eol = |i: usize| {
            if raw[i].ends_with('\r') || (i + 1 == raw.len() && existing.contains("\r\n")) {
                "\r"
            } else {
                ""
            }
        };
        let (at, ending) = match region {
            Some((_, close)) => (close, eol(close)),
            None => match lines.iter().rposition(|l| !l.trim().is_empty()) {
                Some(last) => (last, eol(last)),
                None => return Some(format!("{}\n", kept.join("\n"))),
            },
        };
        let mut out: Vec<String> = raw[..=at]
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        // A last line without its ending gets the file's before more follow.
        if !out[at].ends_with(ending) {
            out[at].push_str(ending);
        }
        out.push(ending.to_string());
        out.extend(kept.iter().map(|l| format!("{l}{ending}")));
        let after = &raw[at + 1..];
        match region {
            // Before the line that follows the region, one blank line.
            Some(_) if !blank(at + 1) => out.push(ending.to_string()),
            Some(_) => {}
            // At the end, the blank lines that were there go.
            None => {
                out.push(String::new());
                return Some(out.join("\n")).filter(|o| o != existing);
            }
        }
        if after.is_empty() {
            out.push(String::new());
        }
        out.extend(after.iter().map(std::string::ToString::to_string));
        let out = out.join("\n");
        (out != existing).then_some(out)
    }

    /// Whether `lines` hold, outside the region and every block, a run of
    /// Markdown blocks that means the same as the `kept` lines, read as
    /// [`meaning::block`] reads a region's block: the runs are the lines
    /// between blank lines outside fenced code, and a run of them is at
    /// most as many runs as the text has lines that are not blank, because a
    /// formatter that adds blank lines between blocks adds no lines of text.
    // sokf-implements: KIT-27_AC-4
    fn held_by_meaning(
        name: &str,
        lines: &[&str],
        region: Option<(usize, usize)>,
        blocks: &[Block],
        kept: &[&str],
    ) -> bool {
        let fenced = fenced(lines);
        let inside = |k: usize| {
            region.is_some_and(|(o, c)| o <= k && k <= c)
                || blocks.iter().any(|b| b.open <= k && k <= b.close)
        };
        // Each run's first and last line, and the stretch between blocks
        // that it stands in.
        let mut runs: Vec<(usize, usize, usize)> = Vec::new();
        let mut stretch = 0;
        let mut open: Option<usize> = None;
        for k in 0..=lines.len() {
            let separator =
                k == lines.len() || inside(k) || (lines[k].trim().is_empty() && !fenced[k]);
            match (open, separator) {
                (Some(start), true) => {
                    runs.push((start, k - 1, stretch));
                    open = None;
                }
                (None, false) => open = Some(k),
                _ => {}
            }
            if k < lines.len() && inside(k) {
                stretch += 1;
            }
        }
        let want = meaning::block(name, &format!("{}\n", kept.join("\n")));
        let most = kept.iter().filter(|l| !l.trim().is_empty()).count().max(1);
        // A re-layout changes no letter, so a run of runs is read only when
        // it holds as many letters as the text, and the search stops past it.
        let letters = |l: &[&str]| -> usize {
            l.iter()
                .map(|l| l.chars().filter(|c| c.is_alphabetic()).count())
                .sum()
        };
        let need = letters(kept);
        for i in 0..runs.len() {
            let mut have = 0;
            for j in i..runs.len().min(i + most) {
                if runs[j].2 != runs[i].2 {
                    break;
                }
                have += letters(&lines[runs[j].0..=runs[j].1]);
                if have > need {
                    break;
                }
                if have == need {
                    let found = format!("{}\n", lines[runs[i].0..=runs[j].1].join("\n"));
                    if meaning::same(&want, &meaning::block(name, &found)) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Whether LF-normalisable `text` holds a line that is not blank outside
    /// every tool's block: text of the project's own, not only blocks the
    /// tools wrote (HAR-16).
    pub(crate) fn own_text(text: &str) -> bool {
        let text = normalise(text);
        let lines: Vec<&str> = text.split('\n').collect();
        // A name no tool has, so every block is another tool's.
        let blocks = Markers::new("/").blocks(&lines);
        lines.iter().enumerate().any(|(k, l)| {
            !l.trim().is_empty() && !blocks.iter().any(|b| b.open <= k && k <= b.close)
        })
    }

    /// The first line of `text` that is a block's marker, any tool's, opening
    /// or closing, fenced or not: a kept text holds none (KIT-27_AC-7).
    pub(crate) fn marker_line(text: &str) -> Option<String> {
        normalise(text)
            .split('\n')
            .map(str::trim_end)
            .find(|l| opening(l).is_some() || closing(l).is_some())
            .map(str::to_string)
    }

    /// `existing` without the region ([`cut`]), its line endings kept.
    /// `None` when there is no region.
    // sokf-implements: KIT-22_AC-1
    pub(crate) fn remove(&self, existing: &str) -> Option<String> {
        let crlf = existing.contains("\r\n");
        let text = normalise(existing);
        let lines: Vec<&str> = text.split('\n').collect();
        let out = cut(&lines, self.locate(&lines)?);
        Some(if crlf { out.replace('\n', "\r\n") } else { out })
    }
}

/// LF `lines` without the region at `open`..=`close`: its marker lines,
/// what lies between them, and the one blank line before the opening marker
/// that [`Markers::render`] puts there when it appends, or, when no blank
/// line is before it, the blank line after it when the region is at the top
/// of the file or another tool's block follows that line, which a region put
/// before a last block has.
fn cut(lines: &[&str], (open, close): (usize, usize)) -> String {
    let blank = |i: usize| lines.get(i).is_some_and(|l| l.trim().is_empty());
    let block_after = unfenced(lines)
        .get(close + 2)
        .is_some_and(|l| opening(l.trim_end()).is_some());
    let (from, to) = if open > 0 && blank(open - 1) {
        (open - 1, close + 1)
    } else if blank(close + 1) && (open == 0 || block_after) {
        (open, close + 2)
    } else {
        (open, close + 1)
    };
    lines[..from]
        .iter()
        .chain(lines.get(to..).unwrap_or_default())
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests;
