use bevy_clipboard::ClipboardRead;
use bevy_math::Vec2;
use bevy_reflect::Reflect;
use core::ops::Range;
use parley::PlainEditorDriver;
use smol_str::SmolStr;

use crate::editing::ObscuredText;
use crate::TextBrush;

/// A selection within IME preedit text, expressed as byte offsets from the start of the preedit.
///
/// The anchor and focus map directly onto parley's `Selection::new(anchor, focus)` when
/// the preedit is applied. If `anchor == focus`, the selection is a caret.
///
/// This corresponds to [`ImePredit::Commit.cursor`](https://docs.rs/bevy/latest/bevy/prelude/enum.Ime.html#variant.Preedit.field.cursor)
/// from `bevy_window`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct PreeditCursor {
    /// Anchor byte offset within the preedit text.
    pub anchor: usize,
    /// Focus (caret) byte offset within the preedit text.
    pub focus: usize,
}

/// Deferred text input edit and navigation actions applied by the `apply_text_edits` system.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum TextEdit {
    /// Copy the current selection into the clipboard.
    ///
    /// Typically generated in response to copy commands such as Ctrl + C or Cmd + C.
    Copy,
    /// Copy the current selection into the clipboard, and the delete the selected text.
    ///
    /// Typically generated in response to cut commands such as Ctrl + X or Cmd + X.
    Cut,
    /// Paste the current clipboard contents at the cursor. If there is a selection, replaces the selection with the clipboard contents instead.
    ///
    /// Typically generated in response to paste commands such as Ctrl + V or Cmd + V.
    Paste,
    /// Insert a character or string at the cursor. If there is a selection, replaces the selection with the character instead.
    ///
    /// Ordinarily, this is derived from `bevy_input::keyboard::KeyboardInput::logical_key`,
    /// which stores a [`SmolStr`] inside of the `Key::Character` variant, which may represent multiple bytes.
    Insert(SmolStr),
    /// Delete the character behind the cursor.
    /// If there is a selection, deletes the selection instead.
    ///
    /// Typically generated in response to the Backspace key.
    ///
    /// This operation removes an entire Unicode grapheme cluster, which may consist of multiple bytes,
    /// shifting the cursor position accordingly.
    Backspace,
    /// Delete the word behind the cursor.
    /// If there is a selection, deletes the selection instead.
    ///
    /// Typically generated in response to Ctrl + Backspace or Option + Backspace.
    BackspaceWord,
    /// Delete the character at the cursor.
    /// If there is a selection, deletes the selection instead.
    ///
    /// Typically generated in response to the Delete key.
    ///
    /// This operation removes an entire Unicode grapheme cluster, which may consist of multiple bytes,
    /// shifting the cursor position accordingly.
    Delete,
    /// Delete the word at the cursor.
    /// If there is a selection, deletes the selection instead.
    ///
    /// Typically generated in response to Ctrl + Delete or Option + Delete.
    DeleteWord,
    /// Moves the cursor by one position to the left.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to the Left key.
    Left(bool),
    /// Moves the cursor by one position to the right.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to the Right key.
    Right(bool),
    /// Moves the cursor a word to the left.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to Ctrl + Left or Option + Left.
    WordLeft(bool),
    /// Moves the cursor a word to the right.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to Ctrl + Right or Option + Right.
    WordRight(bool),
    /// Moves the cursor up by one visual line.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to the Up key.
    Up(bool),
    /// Moves the cursor down by one visual line.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to the Down key.
    Down(bool),
    /// Moves the cursor to the start of the text.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to Ctrl + Home or Command + Up.
    TextStart(bool),
    /// Moves the cursor to the end of the text.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to Ctrl + End or Command + Down.
    TextEnd(bool),
    /// Moves the cursor to the start of the current hard line.
    /// A hardline is a line separated by a newline character.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to Command + Left.
    HardLineStart(bool),
    /// Moves the cursor to the end of the current hard line.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to Command + Right.
    HardLineEnd(bool),
    /// Moves the cursor to the start of the current visual line.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to the Home key.
    LineStart(bool),
    /// Moves the cursor to the end of the current visual line.
    /// `true` moves and extends selection.
    ///
    /// Typically generated in response to the End key.
    LineEnd(bool),
    /// Collapses the current selection to a caret.
    ///
    /// Typically generated in response to the Escape key.
    CollapseSelection,
    /// Selects all text.
    ///
    /// Typically generated in response to select-all commands such as Ctrl + A or Cmd + A.
    SelectAll,
    /// Selects all text if the current selection is collapsed.
    ///
    /// Typically generated in response to a chain of focus gained by pointer press into
    /// pointer release events.
    SelectAllIfCollapsed,
    /// Moves the cursor to the given point.
    ///
    /// Typically generated in response to a pointer press within the text area.
    MoveToPoint(Vec2),
    /// Selects the word at the given point.
    ///
    /// Typically generated in response to a double-click within the text area.
    SelectWordAtPoint(Vec2),
    /// Selects the line at the given point.
    ///
    /// A line here means a single row of glyphs, all sharing the same baseline.
    ///
    /// Typically generated in response to a triple-click within the text area.
    SelectLineAtPoint(Vec2),
    /// Selects the hard line at the given point.
    ///
    /// A “hard line” is the portion of text between explicit newline characters.
    ///
    /// Typically generated in response to a triple-click within the text area.
    SelectedHardLineAtPoint(Vec2),
    /// Extends the current selection to the given point.
    ///
    /// Typically generated in response to dragging a pointer within the text area.
    ExtendSelectionToPoint(Vec2),
    /// Extends the current selection from the existing anchor to the given point.
    ///
    /// Typically generated in response to shift-clicking within the text area.
    ShiftClickExtension(Vec2),
    /// Set the IME preedit/composing text at the cursor, or clear it if `value` is empty.
    ///
    /// The preedit text is excluded from [`EditableText::value`](crate::EditableText::value).
    /// `cursor` describes the selection within the preedit text, or `None` to hide the cursor.
    ///
    /// Passing an empty `value` clears any in-progress composition; `cursor` is ignored in
    /// that case. Use [`TextEdit::clear_ime_compose`] as a convenience constructor.
    ///
    /// Typically generated in response to [`bevy_window::Ime::Preedit`] events.
    ///
    /// [`bevy_window::Ime::Preedit`]: https://docs.rs/bevy/latest/bevy/prelude/enum.Ime.html#variant.Preedit
    ImeSetCompose {
        /// The current preedit string. An empty string clears the composition.
        value: SmolStr,
        /// Selection within the preedit text, or `None` to hide the cursor.
        cursor: Option<PreeditCursor>,
    },
    /// Accept IME composition and insert `value` at the cursor.
    ///
    /// Clears any in-progress preedit first, then inserts the committed string,
    /// respecting [`EditableText::max_characters`](crate::EditableText::max_characters)
    /// and the `char_filter`.
    ///
    /// Typically generated in response to [`bevy_window::Ime::Commit`] events.
    ///
    /// [`bevy_window::Ime::Commit`]: https://docs.rs/bevy/latest/bevy/prelude/enum.Ime.html#variant.Commit
    ImeCommit {
        /// The committed text to insert at the cursor.
        value: SmolStr,
    },
}

impl TextEdit {
    /// Convenience constructor for a [`TextEdit::ImeSetCompose`] that clears the preedit.
    pub fn clear_ime_compose() -> Self {
        Self::ImeSetCompose {
            value: SmolStr::new_inline(""),
            cursor: None,
        }
    }

    /// Apply the [`TextEdit`] to the text editor driver.
    ///
    /// Note that some edits, such as [`TextEdit::Paste`], may need to be deferred across frames due to asynchronous clipboard I/O.
    /// For proper handling of deferred edits, use [`EditableText::apply_pending_edits`](super::EditableText::apply_pending_edits) instead,
    /// which manages the queuing and application of edits by storing them in the [`EditableText`](super::EditableText) component.
    pub fn apply<'a>(
        self,
        driver: &'a mut PlainEditorDriver<TextBrush>,
        clipboard: &mut bevy_clipboard::Clipboard,
        max_characters: Option<usize>,
        char_filter: impl Fn(char) -> bool,
    ) {
        match self {
            TextEdit::Copy => {
                if let Some(text) = driver.editor.selected_text()
                    && let Err(e) = clipboard.set_text(text)
                {
                    bevy_log::warn!("Failed to write selection to clipboard: {e:?}");
                }
            }
            TextEdit::Cut => {
                if let Some(text) = driver.editor.selected_text() {
                    match clipboard.set_text(text) {
                        Ok(()) => driver.delete(),
                        Err(e) => bevy_log::warn!("Failed to write selection to clipboard: {e:?}"),
                    }
                }
            }
            TextEdit::Paste => {
                // It's nice to be able to provide apply as a public method, but Paste is a little buggy.
                // We'll try our best since that works on native, but we should warn users away from doing so.
                bevy_log::warn_once!("Directly applying a Paste edit is not recommended, as it cannot defer asynchronous clipboard reads.
                    For proper handling of async clipboard operations, use `EditableText::apply_pending_edits` instead.");

                let mut read = clipboard.fetch_text();
                poll_and_apply_paste(&mut read, driver, None, max_characters, char_filter);
            }
            TextEdit::Insert(text) => {
                let _ = insert_filtered(driver, text.as_str(), max_characters, char_filter);
            }
            TextEdit::Backspace => driver.backdelete(),
            TextEdit::BackspaceWord => driver.backdelete_word(),
            TextEdit::Delete => driver.delete(),
            TextEdit::DeleteWord => driver.delete_word(),
            TextEdit::Left(false) => driver.move_left(),
            TextEdit::Right(false) => driver.move_right(),
            TextEdit::WordLeft(false) => driver.move_word_left(),
            TextEdit::WordRight(false) => driver.move_word_right(),
            TextEdit::Up(false) => driver.move_up(),
            TextEdit::Down(false) => driver.move_down(),
            TextEdit::TextStart(false) => driver.move_to_text_start(),
            TextEdit::TextEnd(false) => driver.move_to_text_end(),
            TextEdit::HardLineStart(false) => driver.move_to_hard_line_start(),
            TextEdit::HardLineEnd(false) => driver.move_to_hard_line_end(),
            TextEdit::LineStart(false) => driver.move_to_line_start(),
            TextEdit::LineEnd(false) => driver.move_to_line_end(),
            TextEdit::Left(true) => driver.select_left(),
            TextEdit::Right(true) => driver.select_right(),
            TextEdit::WordLeft(true) => driver.select_word_left(),
            TextEdit::WordRight(true) => driver.select_word_right(),
            TextEdit::Up(true) => driver.select_up(),
            TextEdit::Down(true) => driver.select_down(),
            TextEdit::TextStart(true) => driver.select_to_text_start(),
            TextEdit::TextEnd(true) => driver.select_to_text_end(),
            TextEdit::HardLineStart(true) => driver.select_to_hard_line_start(),
            TextEdit::HardLineEnd(true) => driver.select_to_hard_line_end(),
            TextEdit::LineStart(true) => driver.select_to_line_start(),
            TextEdit::LineEnd(true) => driver.select_to_line_end(),
            TextEdit::CollapseSelection => driver.collapse_selection(),
            TextEdit::SelectAll => driver.select_all(),
            TextEdit::SelectAllIfCollapsed => {
                if driver.editor.raw_selection().is_collapsed() {
                    driver.select_all();
                }
            }
            TextEdit::MoveToPoint(point) => driver.move_to_point(point.x, point.y),
            TextEdit::SelectWordAtPoint(point) => driver.select_word_at_point(point.x, point.y),
            TextEdit::SelectLineAtPoint(point) => driver.select_line_at_point(point.x, point.y),
            TextEdit::SelectedHardLineAtPoint(point) => {
                driver.select_hard_line_at_point(point.x, point.y);
            }
            TextEdit::ExtendSelectionToPoint(point) => {
                driver.extend_selection_to_point(point.x, point.y);
            }
            TextEdit::ShiftClickExtension(point) => driver.shift_click_extension(point.x, point.y),
            TextEdit::ImeSetCompose { value, cursor } => {
                if value.is_empty() {
                    driver.clear_compose();
                } else {
                    let cursor = cursor.map(|c| (c.anchor, c.focus));
                    driver.set_compose(&value, cursor);
                }
            }
            TextEdit::ImeCommit { value: text } => {
                driver.clear_compose();
                if text.chars().all(&char_filter)
                    && max_characters.is_none_or(|max| {
                        driver.editor.text().chars().count() + text.chars().count() <= max
                    })
                {
                    driver.insert_or_replace_selection(text.as_str());
                }
            }
        }
    }
}

/// Reason an [`insert_filtered`] call was rejected.
///
/// The two branches matter to callers (paste warns on [`CharFilter`](Self::CharFilter) but
/// not on [`MaxLength`](Self::MaxLength)), so a bool return wouldn't suffice.
enum InsertRejection {
    /// At least one character failed the user-supplied filter.
    CharFilter,
    /// The insertion would exceed `max_characters`.
    MaxLength,
}

/// Insert (or replace the current selection with) `text`, subject to the char filter and
/// `max_characters`.
///
/// Shared by [`TextEdit::Insert`] and [`TextEdit::Paste`] paths to ensure consistent behavior.
fn insert_filtered(
    driver: &mut PlainEditorDriver<TextBrush>,
    text: &str,
    max_characters: Option<usize>,
    char_filter: impl Fn(char) -> bool,
) -> Result<(), InsertRejection> {
    if !text.chars().all(char_filter) {
        return Err(InsertRejection::CharFilter);
    }
    if let Some(max) = max_characters {
        let select_len = driver
            .editor
            .selected_text()
            .map(str::chars)
            .map(Iterator::count)
            .unwrap_or(0);
        if max < driver.editor.text().chars().count() - select_len + text.chars().count() {
            return Err(InsertRejection::MaxLength);
        }
    }
    driver.insert_or_replace_selection(text);
    Ok(())
}

/// Polls a clipboard read and, if ready, applies the resulting text as a paste.
///
/// Returns `true` when the read has resolved (applied, filter-rejected, or errored)
/// and the caller should move on.
/// Returns `false` when the read is still pending
/// and the caller should hold onto the [`ClipboardRead`] to poll again on a later frame.
pub(crate) fn poll_and_apply_paste(
    read: &mut ClipboardRead,
    driver: &mut PlainEditorDriver<TextBrush>,
    mut obscured: Option<&mut ObscuredText>,
    max_characters: Option<usize>,
    char_filter: impl Fn(char) -> bool,
) -> bool {
    match read.poll_result() {
        Some(Ok(text)) => {
            let rejection = match &mut obscured {
                Some(obscured) => {
                    insert_filtered_obscured(driver, obscured, &text, max_characters, char_filter)
                }
                None => insert_filtered(driver, &text, max_characters, char_filter),
            };
            if matches!(rejection, Err(InsertRejection::CharFilter)) {
                bevy_log::debug!(
                    "Paste rejected: clipboard contents contained characters not allowed by the char filter."
                );
            }
            true
        }
        Some(Err(e)) => {
            bevy_log::warn!("Failed to read clipboard for paste: {e:?}");
            true
        }
        None => false,
    }
}

/// Apply a [`TextEdit`] to an obscured (password-style) input.
///
/// The editor buffer holds one mask character per real character, so cursor
/// movement, selection, and hit-testing all work on the masked text
/// unchanged. Content edits are applied to the masked buffer via the driver
/// and mirrored onto the real text by character position — the two stay in
/// a strict one-`char`-to-one-`char` correspondence.
///
/// Deviations from plain behavior, all deliberate:
/// - [`TextEdit::Copy`] is a no-op and [`TextEdit::Cut`] only deletes: the
///   hidden text is never placed on the clipboard.
/// - [`TextEdit::ImeSetCompose`] is ignored (preedit would reveal real text
///   inside the masked buffer and break the char correspondence);
///   [`TextEdit::ImeCommit`] still inserts.
/// - Word-wise movement and deletion segment the *mask characters*, not the
///   hidden text — segmenting the real text would leak its word structure.
pub(crate) fn apply_obscured(
    edit: TextEdit,
    driver: &mut PlainEditorDriver<TextBrush>,
    obscured: &mut ObscuredText,
    clipboard: &mut bevy_clipboard::Clipboard,
    max_characters: Option<usize>,
    char_filter: impl Fn(char) -> bool,
) {
    match edit {
        TextEdit::Copy => {}
        TextEdit::Cut => {
            if !driver.editor.raw_selection().is_collapsed() {
                delete_obscured(driver, obscured, TextEdit::Delete);
            }
        }
        TextEdit::Paste => {
            bevy_log::warn_once!("Directly applying a Paste edit is not recommended, as it cannot defer asynchronous clipboard reads.
                For proper handling of async clipboard operations, use `EditableText::apply_pending_edits` instead.");
            let mut read = clipboard.fetch_text();
            poll_and_apply_paste(
                &mut read,
                driver,
                Some(obscured),
                max_characters,
                char_filter,
            );
        }
        TextEdit::Insert(text) => {
            let _ = insert_filtered_obscured(
                driver,
                obscured,
                text.as_str(),
                max_characters,
                char_filter,
            );
        }
        TextEdit::ImeCommit { value } => {
            let _ = insert_filtered_obscured(
                driver,
                obscured,
                value.as_str(),
                max_characters,
                char_filter,
            );
        }
        TextEdit::ImeSetCompose { .. } => {
            bevy_log::debug_once!(
                "IME preedit is not supported on obscured text inputs; composition ignored."
            );
        }
        edit @ (TextEdit::Backspace
        | TextEdit::BackspaceWord
        | TextEdit::Delete
        | TextEdit::DeleteWord) => delete_obscured(driver, obscured, edit),
        // Everything else moves the cursor or selection over the masked
        // buffer; no mirroring required. (The clipboard is unused by these.)
        other => other.apply(driver, clipboard, max_characters, char_filter),
    }
}

/// The current selection of the masked buffer, as a `char` range.
///
/// Exact because the masked buffer consists solely of copies of the mask
/// character: byte offsets are always a multiple of its UTF-8 length.
fn selected_chars(driver: &PlainEditorDriver<TextBrush>, obscured: &ObscuredText) -> Range<usize> {
    let mask_len = obscured.mask_char().len_utf8();
    let bytes = driver.editor.raw_selection().text_range();
    debug_assert!(bytes.start.is_multiple_of(mask_len) && bytes.end.is_multiple_of(mask_len));
    (bytes.start / mask_len)..(bytes.end / mask_len)
}

/// Convert a `char` range into a byte range of `real`.
fn real_byte_range(real: &str, chars: Range<usize>) -> Range<usize> {
    let byte_of = |char_index| {
        real.char_indices()
            .nth(char_index)
            .map(|(byte, _)| byte)
            .unwrap_or(real.len())
    };
    byte_of(chars.start)..byte_of(chars.end)
}

/// Obscured counterpart of [`insert_filtered`]: filter and length-check the
/// *real* text, splice it into the hidden buffer at the selection, and insert
/// mask characters into the editor.
fn insert_filtered_obscured(
    driver: &mut PlainEditorDriver<TextBrush>,
    obscured: &mut ObscuredText,
    text: &str,
    max_characters: Option<usize>,
    char_filter: impl Fn(char) -> bool,
) -> Result<(), InsertRejection> {
    if !text.chars().all(char_filter) {
        return Err(InsertRejection::CharFilter);
    }
    let selection = selected_chars(driver, obscured);
    if let Some(max) = max_characters {
        let current = obscured.text().chars().count();
        if max < current - selection.len() + text.chars().count() {
            return Err(InsertRejection::MaxLength);
        }
    }
    let masked = obscured.mask_of(text);
    let byte_range = real_byte_range(obscured.text(), selection);
    obscured.text_mut().replace_range(byte_range, text);
    driver.insert_or_replace_selection(&masked);
    Ok(())
}

/// Apply a deletion edit to the masked buffer and mirror the removed `char`
/// range onto the real text.
///
/// The removed range is derived from the pre-edit selection plus the change
/// in character count — never by diffing the (uniform) masked text:
/// - a non-collapsed selection is what all four deletions remove;
/// - otherwise backward deletions end at the caret, forward ones start there.
fn delete_obscured(
    driver: &mut PlainEditorDriver<TextBrush>,
    obscured: &mut ObscuredText,
    edit: TextEdit,
) {
    let selection = selected_chars(driver, obscured);
    let before = obscured.text().chars().count();
    let backward = matches!(edit, TextEdit::Backspace | TextEdit::BackspaceWord);
    match edit {
        TextEdit::Backspace => driver.backdelete(),
        TextEdit::BackspaceWord => driver.backdelete_word(),
        TextEdit::Delete => driver.delete(),
        TextEdit::DeleteWord => driver.delete_word(),
        _ => unreachable!("delete_obscured only receives deletion edits"),
    }
    let after = driver.editor.text().chars().count();
    let removed = before - after;
    if removed == 0 {
        return;
    }
    let removed_chars = if !selection.is_empty() {
        debug_assert_eq!(removed, selection.len());
        selection
    } else if backward {
        (selection.start - removed)..selection.start
    } else {
        selection.start..(selection.start + removed)
    };
    let byte_range = real_byte_range(obscured.text(), removed_chars);
    obscured.text_mut().replace_range(byte_range, "");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EditableText;
    use alloc::string::ToString;
    use alloc::sync::Arc;
    use alloc::vec::Vec;
    use parley::{fontique::Blob, FontContext, LayoutContext};

    fn contexts() -> (FontContext, LayoutContext<TextBrush>) {
        let mut font_context = FontContext::new();
        font_context.collection.register_fonts(
            Blob::new(Arc::new(include_bytes!("FiraMono-subset.ttf").to_vec())),
            None,
        );
        (font_context, LayoutContext::new())
    }

    /// Point the editor's default style at the registered test font; without
    /// a resolvable family nothing shapes, and cluster-based operations
    /// (movement, selection) silently no-op.
    fn use_test_font(input: &mut EditableText) {
        input
            .editor
            .edit_styles()
            .insert(parley::StyleProperty::FontFamily(
                parley::FontFamily::named("Fira Mono"),
            ));
    }

    fn obscured_input(mask_char: char) -> EditableText {
        let mut input = EditableText::new_obscured(mask_char);
        use_test_font(&mut input);
        input
    }

    fn apply(input: &mut EditableText, edits: impl IntoIterator<Item = TextEdit>) {
        let (mut font_context, mut layout_context) = contexts();
        let mut clipboard = bevy_clipboard::Clipboard::default();
        input.pending_edits.extend(edits);
        input.apply_pending_edits(
            &mut font_context,
            &mut layout_context,
            &mut clipboard,
            |_| true,
        );
    }

    fn apply_filtered(
        input: &mut EditableText,
        edits: impl IntoIterator<Item = TextEdit>,
        filter: impl Fn(char) -> bool,
    ) {
        let (mut font_context, mut layout_context) = contexts();
        let mut clipboard = bevy_clipboard::Clipboard::default();
        input.pending_edits.extend(edits);
        input.apply_pending_edits(
            &mut font_context,
            &mut layout_context,
            &mut clipboard,
            filter,
        );
    }

    fn insert(text: &str) -> TextEdit {
        TextEdit::Insert(SmolStr::new(text))
    }

    /// The masked buffer must contain exactly one mask char per real char.
    fn assert_masked(input: &EditableText) {
        let obscured = input.obscured.as_ref().expect("obscured input");
        let buffer = input.editor().text().to_string();
        assert!(
            buffer.chars().all(|c| c == obscured.mask_char()),
            "{buffer:?}"
        );
        assert_eq!(buffer.chars().count(), input.value().chars().count());
    }

    #[test]
    fn typing_is_masked_and_value_is_real() {
        let mut input = obscured_input('*');
        apply(&mut input, [insert("hunter2")]);
        assert_eq!(input.value(), "hunter2");
        assert_eq!(input.editor().text().to_string(), "*******");
        assert_masked(&input);
    }

    #[test]
    fn masking_is_per_char_for_multibyte_text() {
        let mut input = EditableText::new_obscured('\u{2022}');
        apply(&mut input, [insert("p\u{e9}\u{1f600}z")]);
        assert_eq!(input.value(), "p\u{e9}\u{1f600}z");
        assert_eq!(input.editor().text().chars().count(), 4);
        assert_masked(&input);
    }

    #[test]
    fn deletions_mirror_by_position() {
        let mut input = obscured_input('*');
        apply(&mut input, [insert("abcd")]);
        // Caret between b and c: ab|cd
        apply(&mut input, [TextEdit::Left(false), TextEdit::Left(false)]);
        apply(&mut input, [TextEdit::Backspace]);
        assert_eq!(input.value(), "acd");
        apply(&mut input, [TextEdit::Delete]);
        assert_eq!(input.value(), "ad");
        assert_masked(&input);
    }

    #[test]
    fn word_deletions_mirror_whatever_the_mask_segments_to() {
        // Word boundaries are computed over the *mask characters* (deliberate:
        // segmenting the real text would leak word structure through word-wise
        // ops). Whatever count a word deletion removes from the mask must come
        // off the same position of the real text.
        let mut input = obscured_input('*');
        apply(&mut input, [insert("correcthorse")]);
        apply(&mut input, [TextEdit::BackspaceWord]);
        let remaining = input.editor().text().chars().count();
        assert!(remaining < 12, "a word deletion must remove something");
        assert_eq!(input.value(), "correcthorse"[..remaining].to_string());
        assert_masked(&input);
        // Repeated word deletions still empty the field.
        for _ in 0..12 {
            apply(&mut input, [TextEdit::BackspaceWord]);
        }
        assert_eq!(input.value(), "");
        assert_masked(&input);
    }

    #[test]
    fn selection_replacement() {
        let mut input = obscured_input('*');
        apply(
            &mut input,
            [insert("old"), TextEdit::SelectAll, insert("new!")],
        );
        assert_eq!(input.value(), "new!");
        assert_masked(&input);
    }

    #[test]
    fn cut_deletes_without_copy_and_copy_is_inert() {
        let mut input = obscured_input('*');
        apply(
            &mut input,
            [insert("secret"), TextEdit::SelectAll, TextEdit::Copy],
        );
        assert_eq!(input.value(), "secret", "copy must not modify");
        // Cut removes the selection; that the clipboard is untouched is
        // enforced by construction (the obscured path never calls set_text
        // on the clipboard) rather than asserted, since headless clipboard
        // state is not reliably observable.
        apply(&mut input, [TextEdit::Cut]);
        assert_eq!(input.value(), "");
        assert_masked(&input);
    }

    #[test]
    fn filter_and_max_characters_apply_to_real_text() {
        let mut input = obscured_input('*');
        input.max_characters = Some(4);
        apply_filtered(&mut input, [insert("abc1")], |c| c.is_ascii_alphabetic());
        assert_eq!(input.value(), "", "rejected inserts leave no residue");
        apply_filtered(&mut input, [insert("abcd"), insert("e")], |c| {
            c.is_ascii_alphabetic()
        });
        assert_eq!(input.value(), "abcd", "max_characters counts real chars");
        assert_masked(&input);
    }

    #[test]
    fn ime_commit_inserts_and_preedit_is_ignored() {
        let mut input = obscured_input('*');
        apply(
            &mut input,
            [
                TextEdit::ImeSetCompose {
                    value: SmolStr::new("\u{306b}"),
                    cursor: None,
                },
                TextEdit::ImeCommit {
                    value: SmolStr::new("\u{65e5}\u{672c}"),
                },
            ],
        );
        assert_eq!(input.value(), "\u{65e5}\u{672c}");
        assert_masked(&input);
    }

    #[test]
    fn toggling_obscured_converts_content() {
        let mut input = EditableText::new("hunter2");
        use_test_font(&mut input);
        input.set_obscured(Some('*'));
        assert_eq!(input.editor().text().to_string(), "*******");
        assert_eq!(input.value(), "hunter2");
        input.set_obscured(None);
        assert_eq!(input.editor().text().to_string(), "hunter2");
        assert_eq!(input.value(), "hunter2");
    }

    #[test]
    fn clear_clears_the_real_text_too() {
        let mut input = obscured_input('*');
        apply(&mut input, [insert("secret")]);
        input.clear();
        assert_eq!(input.value(), "");
        assert_eq!(input.editor().text().to_string(), "");
    }

    #[test]
    fn plain_value_borrows_and_matches() {
        let mut input = EditableText::default();
        use_test_font(&mut input);
        apply(&mut input, [insert("plain")]);
        let value = input.value();
        assert_eq!(value, "plain");
        assert!(matches!(value, alloc::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn movement_edits_stay_in_bounds() {
        let mut input = obscured_input('*');
        let motions: Vec<TextEdit> = alloc::vec![
            TextEdit::TextStart(false),
            TextEdit::Right(true),
            TextEdit::WordRight(true),
            TextEdit::LineEnd(false),
            TextEdit::Backspace,
        ];
        apply(&mut input, [insert("abcdef")]);
        apply(&mut input, motions);
        assert_eq!(input.value(), "abcde");
        assert_masked(&input);
    }
}
