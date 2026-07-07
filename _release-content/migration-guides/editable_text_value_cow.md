---
title: "`EditableText::value` now returns `Cow<str>`"
pull_requests: []
---

`EditableText::value()` previously returned `parley::SplitString`. To support the new
obscured (password-style) mode — where the value is the hidden real text rather than the
editor buffer — it now returns `Cow<'_, str>`.

Most call sites need no change: `Cow<str>` supports `Display`, `to_string()`, `.chars()`,
and comparison with `&str` just like `SplitString` did. Code that iterated the
`SplitString` segments to avoid allocation can simply use the `Cow` as a `&str`:

```rust
// 0.19
for sub_str in editable_text.value() {
    text.push_str(sub_str);
}

// 0.20
text.push_str(&editable_text.value());
```

The `Cow` is borrowed except while an IME preedit splits the buffer, so the common case
still does not allocate.
