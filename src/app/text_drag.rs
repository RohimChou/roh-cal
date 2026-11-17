//! Dragging over text with the mouse to copy it: where the drag is, and the text it went over. Used where
//! text can be copied: the day popup's titles (`App::day_drag`) and the help page's file paths
//! (`App::help_drag`)

/// A spot in a list of texts (e.g. the day popup's titles, `App::day_items`; the help page's paths,
/// `App::help_files`): the `char`-th character of the `item`-th text. Sorting goes in reading order: by
/// item, then by character
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextSpot {
    pub item: usize,
    pub char: usize,
}

/// Text being dragged over with the mouse: the character where the left button went down, and once the
/// mouse has moved with it held, the character it is over now
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDrag {
    pub anchor: TextSpot,
    pub head: Option<TextSpot>,
}

impl TextDrag {
    /// The text dragged over, from its first to its last character (both included, in reading order);
    /// `None` until the mouse moves with the button held
    pub fn range(self) -> Option<(TextSpot, TextSpot)> {
        let head = self.head?;
        Some((self.anchor.min(head), self.anchor.max(head)))
    }
}

/// The part of `texts` from `first` to `last` (both included): each text's part on its own line, e.g.
/// 「iage bugs\nTeam」 from Triage bugs' 「i」 to Team sync's 「m」
pub fn text_between(texts: &[&str], first: TextSpot, last: TextSpot) -> String {
    let lines: Vec<String> = (first.item..=last.item)
        .filter_map(|index| {
            let text = texts.get(index)?;
            let from = if index == first.item { first.char } else { 0 };
            let to = if index == last.item {
                last.char + 1
            } else {
                usize::MAX
            };
            Some(text.chars().take(to).skip(from).collect())
        })
        .collect();
    lines.join("\n")
}
