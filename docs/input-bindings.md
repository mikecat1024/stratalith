# Editor input bindings

If you embed the browser editor with `mountEditor`, the bindings below are already active. For example, the browser normally reports Shift+Enter as `insertLineBreak`, and the adapter turns that into a hard break. Adding a second Enter handler can apply the edit twice. [Browser entry point](../packages/browser/src/index.ts) · [Browser input adapter](../packages/browser/src/editor.ts)

If you embed the Rust engine directly, your UI chooses the bindings and sends commands with the current selection. Use the following defaults to match the examples. **Mod** means Command on macOS and Ctrl on Windows or Linux. [GPUI example](../example/gpui/src/main.rs) · [Rust commands](../crates/stratalith-core/src/engine.rs)

| Editing operation | Recommended key | Rust command (`Command::`) | Input handled by the browser adapter |
| --- | --- | --- | --- |
| Start a new paragraph, continue a list, or insert a newline in code | Enter | `Enter` | `insertParagraph` |
| Insert a hard break within a paragraph, or exit a code block | Shift+Enter | `InsertHardBreak` | `insertLineBreak` |
| Indent a list item | Tab | `IndentListItem` | Tab or `formatIndent` |
| Outdent a list item | Shift+Tab | `OutdentListItem` | Shift+Tab or `formatOutdent` |
| Delete the selection or delete backward | Backspace | `Backspace` | `deleteContentBackward` |
| Delete the selection or delete forward | Delete (forward delete) | `Delete` | `deleteContentForward` |
| Undo | Mod+Z | `Undo` | Shortcut or `historyUndo` |
| Redo | Mod+Shift+Z, also Ctrl+Y | `Redo` | Shortcut or `historyRedo` |

The engine resolves Enter and Shift+Enter using the selection and block type. A native host can dispatch the same command in a paragraph, list item, or code block. For Tab and Shift+Tab, the browser adapter prevents default focus navigation only when the list edit succeeds. [Engine dispatch](../crates/stratalith-core/src/engine.rs) · [Browser shortcut handler](../packages/browser/src/editor.ts)

## Text, clipboard, and selection

| Operation | What a native host should do | What the browser editor handles |
| --- | --- | --- |
| Type text or confirm IME input | Use the platform text-input callback. Send committed text with `InsertText` for the current selection or `ReplaceRange` for an explicit replacement range. Let the IME handle Enter while composing. | Reads text and target ranges from input events and commits composed text on `compositionend`. |
| Delete a word or a line | Resolve the platform-specific deletion range and send `ReplaceRange` with empty text. | Uses the browser's target range or derives it from the native selection. Also uses range deletion when a character spans multiple engine offsets. |
| Copy or cut (Mod+C / Mod+X) | Extract and serialize the selected fragment to the clipboard. For cut, then delete the selection with `ReplaceRange`. | Handles the `copy` and `cut` events and writes the selection as plain-text Markdown. |
| Paste (Mod+V) | Read clipboard text and pass it with the target selection to `paste_command`. Apply the returned command, if any. | Handles paste and prefers `text/markdown` when available, followed by `text/plain`. |
| Move the caret, extend the selection, or select all | Use native navigation keys. Shift+Arrow extends the selection and Mod+A selects the document. Update the engine selection. | The browser moves the DOM selection. The adapter reads it when dispatching the next edit. |

The [GPUI example](../example/gpui/src/main.rs) shows the native key, clipboard, and text-input callbacks. The [Markdown paste helper](../crates/stratalith-markdown/src/paste.rs) chooses how to insert clipboard text.
