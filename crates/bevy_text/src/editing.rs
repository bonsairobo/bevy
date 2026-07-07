//! A simple text input widget for Bevy UI.
//!
//! The [`EditableText`] widget is an undecorated rectangular text input field,
//! which allows users to input and edit text within a Bevy UI application.
//! Every [`EditableText`] component is also a [`Node`](https://docs.rs/bevy/latest/bevy/prelude/struct.Node.html) in the Bevy UI hierarchy,
//! allowing you to position and size it using standard Bevy UI layout techniques.
//! You can think of it as the editable equivalent of [`Text`](https://docs.rs/bevy/latest/bevy/prelude/struct.Text.html),
//! and components such as [`TextFont`] and [`TextColor`] can be used to style it.
//!
//! [`EditableText`] supports the following functionality:
//!
//! - Text entry
//! - Keyboard-driven cursor movement: arrow keys, Home/End, and word-level shortcuts (Ctrl/Alt+arrow)
//! - Shift+arrow and Shift+word-arrow to extend the selection by character or word
//! - Backspace and delete operations, both for single characters and words
//! - Clipboard operations (copy, cut, paste) — requires the `system_clipboard` feature for OS clipboard integration
//! - Click to place cursor; click and drag to extend the selection
//! - Multi-click: double-click to select a word, triple-click to select a line
//! - Optional select-all on focus via the `SelectAllOnFocus` component
//! - Per-character input filtering via the [`EditableTextFilter`] component
//! - Max character limits via [`EditableText::max_characters`]
//! - Cursor blinking
//! - Newline support for multi-line input
//! - Soft-wrapping of long lines
//! - Vertical scrolling for multi-line input
//! - Horizontal scrolling for long lines
//! - Input Method Editor (IME) support for complex scripts (Japanese, Chinese, Korean, etc.)
//! - Bidirectional text support (e.g., mixing left-to-right and right-to-left scripts)
//! - Input consumption (preventing other systems from receiving keyboard input events when the text input is focused)
//! - Password-style character masking via [`EditableText::new_obscured`] / [`EditableText::set_obscured`].
//!   While obscured: clipboard copy/cut never receive the hidden text, and IME preedit is disabled
//!   (committed IME text is still inserted).
//!
//! You might use this widget as the basis for text input fields in forms, chat boxes, for naming characters,
//! or any other scenario where you want to extract an unformatted text string from the user.
//!
//! Reusable widgets that build on top of this basic text input field (as might be found in Bevy's Feathers UI framework),
//! will typically combine this widget with additional UI elements such as borders, backgrounds, and labels,
//! creating a multi-entity widget that matches the semantics and visual appearance required by the application.
//!
//! ## Handling user input
//!
//! User input is handled via a plugin in `bevy_ui_widgets`:
//! [`bevy_text`](crate) is not aware of input events directly.
//!
//! With the correct plugin enabled, when an [`EditableText`] entity is focused,
//! keyboard input events are captured and processed into [`TextEdit`] actions.
//!
//! ## Limitations
//!
//! The formatting of the text is uniform throughout the entire input field.
//! As a result, rich text-editing is out-of-scope:
//! this widget is not intended to form the basis for a full-featured text editor.
//!
//! Similarly, this widget is "headless": it has no built-in styling, and is intended to be used
//! with a themed UI framework of your choice (e.g. Feathers). This means that no text boxes, borders, or other
//! visual elements are provided by default, and must be added separately using Bevy UI entities / components,
//! and any reactive styling (e.g., focus/hover states) must also be implemented separately.
//!
//! However, the following features are planned but currently not implemented:
//!
//! - Placeholder text (displayed when the input is empty)
//! - Undo/redo functionality
//! - Text validation (e.g., email format, numeric input)
//! - Mobile pop-up keyboard support
//! - Overwrite mode (typically toggled by the `Insert` key)
//! - AccessKit integration for screen readers and other assistive technologies
//! - World-space text input
//! - Text form submission handling
//!
//! If you require any of these features, please consider contributing it to the crate,
//! one feature at a time!
// Note: this logic is in `bevy_text`, rather than higher up in `bevy_ui` or `bevy_ui_widgets`,
// because doing so allows us to process `EditableText` in the various systems provided by `bevy_text`
// and `bevy_ui`, such as text layout and font management.

use crate::{
    text_edit::{poll_and_apply_paste, TextEdit},
    FontCx, FontHinting, LayoutCx, LineHeight, TextBrush, TextColor, TextFont, TextLayout,
};
use alloc::borrow::Cow;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use bevy_clipboard::ClipboardRead;
use bevy_derive::{Deref, DerefMut};
use bevy_ecs::prelude::*;
use core::time::Duration;
use parley::{FontContext, LayoutContext, PlainEditor, SplitString};

/// A plain-text text input field.
///
/// Please see this module docs for more details on usage and functionality.
///
/// Note that text editing operations are trickier than they might first appear,
/// due to the complexities of Unicode text handling.
///
/// As a result, we store an internal [`PlainEditor`] instance,
/// which manages both the text content and the cursor position,
/// and provides methods for applying text edits and cursor movements correctly
/// according to Unicode rules.
#[derive(Component, Clone)]
#[require(
    TextLayout,
    TextFont,
    TextColor,
    LineHeight,
    FontHinting,
    EditableTextGeneration
)]
pub struct EditableText {
    /// A [`parley::PlainEditor`], tracking both the text content and cursor position.
    ///
    /// This serves as an analogue to [`ComputedTextBlock`](crate::ComputedTextBlock) for editable text.
    ///
    /// In most cases, you should queue text edits via the [`EditableText::queue_edit`] method instead of directly manipulating the editor,
    /// and then allow the [`apply_text_edits`] system to apply the edits at the appropriate time in the update cycle.
    ///
    /// Note that many more complex editing operations require working with [`PlainEditor::driver`].
    /// These operations should generally be batched together to avoid redundant layout work.
    // The B: Brush generic here must match the brush used by `ComputedTextBlock` to ensure that the font system is compatible.
    pub editor: PlainEditor<TextBrush>,
    /// Text edit actions that have been requested but not yet applied.
    ///
    /// These edits are processed in first-in, first-out order.
    pub pending_edits: Vec<TextEdit>,
    /// A paste operation that is awaiting clipboard I/O.
    ///
    /// On platforms where system clipboard reads are asynchronous (currently wasm32), a
    /// [`TextEdit::Paste`] may not resolve in the same frame it was queued.
    ///
    /// While this field is `Some`, [`apply_pending_edits`](Self::apply_pending_edits) waits for this to resolve,
    /// rather than draining further edits, so that everything after the paste stays correctly ordered *behind* it.
    // TODO: this may cause unexpected stalls if the clipboard read takes too long. We may want to add a timeout.
    pub pending_paste: Option<ClipboardRead>,
    /// Cursor width, relative to font size
    pub cursor_width: f32,
    /// Cursor blink period in seconds.
    pub cursor_blink_period: Duration,
    /// Maximum number of characters the text input can contain.
    ///
    /// Edits which would cause the length to exceed the maximum are ignored.
    /// Does not stop setting a string longer than the maximum using `set_text`.
    pub max_characters: Option<usize>,
    /// Sets the input’s height in number of visible lines.
    pub visible_lines: Option<f32>,
    /// Sets the input's width in number of visible glyphs.
    /// For proportional fonts the final size is the given value times the "0" advance width.
    pub visible_width: Option<f32>,
    /// Allow new lines
    pub allow_newlines: bool,
    /// When `Some`, the input is obscured (password-style): the editor's
    /// buffer — and therefore the layout, cursor, selection, and hit-testing —
    /// contains one mask character per real character, while the real text is
    /// kept in the [`ObscuredText`] and returned by [`EditableText::value`].
    ///
    /// Prefer [`EditableText::new_obscured`] or [`EditableText::set_obscured`]
    /// over mutating this directly, so buffer and real text stay in sync.
    pub obscured: Option<ObscuredText>,
}

impl Default for EditableText {
    fn default() -> Self {
        Self {
            // Defaults selected to match `Text::default()`
            editor: PlainEditor::new(100.),
            pending_edits: Vec::new(),
            pending_paste: None,
            cursor_width: 0.2,
            cursor_blink_period: Duration::from_secs(1),
            max_characters: None,
            visible_lines: Some(1.),
            visible_width: None,
            allow_newlines: false,
            obscured: None,
        }
    }
}

/// The hidden side of an obscured (password-style) [`EditableText`]: the real
/// text, and the character standing in for each of its `char`s on screen.
///
/// The masked editor buffer and `real` are kept in sync by
/// [`apply_text_edits`]; editing the [`PlainEditor`] buffer directly on an
/// obscured input desynchronizes them.
#[derive(Clone)]
pub struct ObscuredText {
    mask_char: char,
    real: String,
}

impl ObscuredText {
    /// An empty obscured text using `mask_char` (commonly `'•'` or `'*'`;
    /// pick one your font supplies).
    pub fn new(mask_char: char) -> Self {
        Self {
            mask_char,
            real: String::new(),
        }
    }

    /// The character shown in place of each real character.
    pub fn mask_char(&self) -> char {
        self.mask_char
    }

    /// The real (hidden) text.
    pub fn text(&self) -> &str {
        &self.real
    }

    pub(crate) fn text_mut(&mut self) -> &mut String {
        &mut self.real
    }

    /// One mask character per `char` of `real`.
    pub(crate) fn mask_of(&self, real: &str) -> String {
        core::iter::repeat_n(self.mask_char, real.chars().count()).collect()
    }
}

impl EditableText {
    /// Creates a new `EditableText` with its buffer already containing some initial text and
    /// its cursor positioned at the end.
    pub fn new(initial_text: impl AsRef<str>) -> Self {
        let mut editable_text = Self::default();
        editable_text.editor.set_text(initial_text.as_ref());
        editable_text.queue_edit(TextEdit::TextEnd(false));
        editable_text
    }

    /// Creates an empty obscured (password-style) `EditableText`.
    ///
    /// The layout only ever sees `mask_char`, so glyph widths cannot leak the
    /// hidden characters; [`EditableText::value`] returns the real text.
    /// See the module docs for the obscured-mode behavior of clipboard and IME.
    pub fn new_obscured(mask_char: char) -> Self {
        Self {
            obscured: Some(ObscuredText::new(mask_char)),
            ..Default::default()
        }
    }

    /// Turns obscuring on (`Some(mask_char)`) or off (`None`), converting any
    /// existing content.
    ///
    /// Prefer toggling between user edits: queued [`TextEdit`]s are applied
    /// against the new mode.
    pub fn set_obscured(&mut self, mask_char: Option<char>) {
        match (mask_char, self.obscured.take()) {
            (Some(mask_char), previous) => {
                let real = match previous {
                    Some(previous) => previous.real,
                    None => self.editor.text().to_string(),
                };
                let obscured = ObscuredText {
                    mask_char,
                    real: String::new(),
                };
                self.editor.set_text(&obscured.mask_of(&real));
                self.obscured = Some(ObscuredText { real, ..obscured });
            }
            (None, Some(previous)) => self.editor.set_text(&previous.real),
            (None, None) => {}
        }
    }

    /// Access the internal [`PlainEditor`].
    pub fn editor(&self) -> &PlainEditor<TextBrush> {
        &self.editor
    }

    /// Mutably access the internal [`PlainEditor`].
    ///
    pub fn editor_mut(&mut self) -> &mut PlainEditor<TextBrush> {
        &mut self.editor
    }

    /// Get the current text input.
    ///
    /// For an obscured input this is the *real* (hidden) text, not the mask
    /// characters in the editor buffer.
    ///
    /// Borrowed unless the buffer is split by an in-progress IME preedit.
    pub fn value(&self) -> Cow<'_, str> {
        if let Some(obscured) = &self.obscured {
            return Cow::Borrowed(obscured.text());
        }
        split_string_cow(self.editor.text())
    }

    /// Queue a [`TextEdit`] action to be applied later by the [`apply_text_edits`] system.
    pub fn queue_edit(&mut self, edit: TextEdit) {
        self.pending_edits.push(edit);
    }

    /// Applies all [`TextEdit`]s in `pending_edits` immediately, updating the [`PlainEditor`] text / cursor state accordingly.
    ///
    /// [`FontContext`] should be gathered from the [`FontCx`] resource, and [`LayoutContext`] should be gathered from the [`LayoutCx`] resource.
    ///
    /// On platforms with async clipboard reads (wasm32), a [`TextEdit::Paste`] whose
    /// contents aren't yet available acts as a barrier: this call parks the in-flight
    /// read on [`EditableText`] and leaves the remaining edits queued in order. Each
    /// subsequent frame re-polls the read, and processing resumes once it resolves.
    /// On native targets clipboard reads are synchronous, so this barrier collapses.
    pub fn apply_pending_edits(
        &mut self,
        font_context: &mut FontContext,
        layout_context: &mut LayoutContext<TextBrush>,
        clipboard: &mut bevy_clipboard::Clipboard,
        char_filter: impl Fn(char) -> bool,
    ) {
        let Self {
            editor,
            pending_edits,
            pending_paste,
            max_characters,
            obscured,
            ..
        } = self;

        let mut driver = editor.driver(font_context, layout_context);

        // First: resolve any paste carried over from a previous frame. If it's still
        // pending, hold the remaining edits (untouched in `pending_edits`) for next frame
        // so ordering relative to the paste is preserved.
        if let Some(mut read) = pending_paste.take()
            && !poll_and_apply_paste(
                &mut read,
                &mut driver,
                obscured.as_mut(),
                *max_characters,
                &char_filter,
            )
        {
            *pending_paste = Some(read);
            return;
        }

        // Drain edits one at a time. A paste that resolves synchronously (always the case
        // on native) applies immediately, but a still-pending paste stashes its `ClipboardRead` and
        // requeues the *remaining* edits, so this loop continually requeues the pending paste until it resolves.
        let mut edits = core::mem::take(pending_edits).into_iter();
        while let Some(edit) = edits.next() {
            match edit {
                TextEdit::Paste => {
                    let mut read = clipboard.fetch_text();
                    if !poll_and_apply_paste(
                        &mut read,
                        &mut driver,
                        obscured.as_mut(),
                        *max_characters,
                        &char_filter,
                    ) {
                        *pending_paste = Some(read);
                        pending_edits.extend(edits);
                        return;
                    }
                }
                other => match obscured.as_mut() {
                    Some(obscured) => crate::text_edit::apply_obscured(
                        other,
                        &mut driver,
                        obscured,
                        clipboard,
                        *max_characters,
                        &char_filter,
                    ),
                    None => other.apply(&mut driver, clipboard, *max_characters, &char_filter),
                },
            }
        }
    }

    /// Clears the input's text buffer and any pending edits.
    ///
    /// Also drops any in-flight paste. The underlying clipboard read task
    /// will still complete, but its result is discarded.
    pub fn clear(&mut self) {
        self.editor.set_text("");
        self.pending_edits.clear();
        self.pending_paste = None;
        if let Some(obscured) = &mut self.obscured {
            obscured.text_mut().clear();
        }
    }

    /// Is the IME currently composing text for this input?
    ///
    /// Some behavior (e.g. "submit on Enter") may want to be suppressed while the IME is active
    /// to avoid interrupting the user's composition.
    pub fn is_composing(&self) -> bool {
        self.editor.is_composing()
    }
}

/// Join a [`SplitString`] into a `Cow`, borrowing in the common contiguous case.
fn split_string_cow(split: SplitString<'_>) -> Cow<'_, str> {
    let mut parts = split.into_iter();
    let first = parts.next().unwrap_or("");
    let second = parts.next().unwrap_or("");
    if second.is_empty() {
        Cow::Borrowed(first)
    } else {
        Cow::Owned([first, second].concat())
    }
}

/// Wrapper around a `parley::Generation`. Used to track when `TextLayoutInfo` is stale and needs reupdating.
/// The initial `Generation` of the `PlainEditor` is not equal to the default `Generation` value, so the
/// `TextLayoutInfo` will always be given an initial update.
#[derive(Component, PartialEq, Eq, Default, Clone, Copy, Deref, DerefMut)]
pub struct EditableTextGeneration(parley::Generation);

/// Sets a per-character filter for this text input. Insert and paste edits are ignored if the filter rejects any character.
///
/// The filter does not apply to characters already within the `EditableText`'s text buffer.
#[derive(Component, Clone, Default)]
pub struct EditableTextFilter(Option<Arc<dyn Fn(char) -> bool + Send + Sync + 'static>>);

impl EditableTextFilter {
    /// Create a new `EditableTextFilter` from the given filter function.
    pub fn new(filter: impl Fn(char) -> bool + Send + Sync + 'static) -> Self {
        Self(Some(Arc::new(filter)))
    }
}

/// Applies pending text edit actions to all [`EditableText`] widgets.
pub fn apply_text_edits(
    mut query: Query<(
        Entity,
        &mut EditableText,
        Option<&EditableTextFilter>,
        &EditableTextGeneration,
    )>,
    mut font_context: ResMut<FontCx>,
    mut layout_context: ResMut<LayoutCx>,
    mut clipboard: ResMut<bevy_clipboard::Clipboard>,
    mut commands: Commands,
) {
    for (entity, mut editable_text, filter, generation) in query.iter_mut() {
        // `pending_paste` can hold a cross-frame paste even when no new edits are queued,
        // so check for either before doing work.
        if !editable_text.pending_edits.is_empty() || editable_text.pending_paste.is_some() {
            editable_text.apply_pending_edits(
                &mut font_context,
                &mut layout_context.0,
                &mut clipboard,
                match filter {
                    Some(EditableTextFilter(Some(filter))) => filter.as_ref(),
                    _ => &|_| true,
                },
            );
        }

        if **generation != editable_text.editor.generation() {
            commands.trigger(TextEditChange { entity });
        }
    }
}

/// Triggered after applying all pending [`TextEdit`]s to the [`EditableText`] by [`apply_text_edits`].
///
/// As [`TextEdit`] includes cursor motions, this will be emitted even if [`EditableText::value`] is unchanged.
#[derive(EntityEvent)]
pub struct TextEditChange {
    entity: Entity,
}
