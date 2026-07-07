---
title: Password-style masking for `EditableText`
authors: ["@bonsairobo"]
pull_requests: []
---

`EditableText` can now obscure its content, password-field style:

```rust
// A fresh password input, displaying one '*' per typed character:
EditableText::new_obscured('*')

// Or toggle an existing input (e.g. a "show password" checkbox):
editable_text.set_obscured(None);
```

While obscured, the editor's buffer — and therefore the layout, cursor, selection, and
click hit-testing — contains only mask characters, so glyph widths cannot leak what was
typed. The real text is tracked separately and returned by `EditableText::value()`.

Obscured inputs deliberately behave differently in a few ways:

- Copy is a no-op and Cut only deletes: the hidden text is never placed on the clipboard.
  (Paste works as usual.)
- IME preedit is disabled, since composition would reveal real text inside the masked
  buffer; committed IME text is still inserted.
- Word-wise movement and deletion segment the mask characters rather than the hidden
  text, so word structure cannot be probed through word-jump shortcuts.

Choose a mask character your font provides — `'*'` is safe everywhere, `'•'` is prettier
where available.
