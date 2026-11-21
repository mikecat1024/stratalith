import { renderEditableDocument } from "./rich-text.js";
import { type DocumentSelection, EditCommand, type loadMarkdown } from "./markdown.js";

type Markdown = Awaited<ReturnType<typeof loadMarkdown>>;
type ReadSelection = ReturnType<typeof renderEditableDocument>;
type InputEdit = Parameters<Markdown["edit"]>;

// The browser resolves word boundaries and visual lines using the current layout.
function deletionTarget(type: string, before: DocumentSelection, read: ReadSelection) {
  const native = document.getSelection();
  if (!native?.isCollapsed || !native.focusNode) {
    return before;
  }
  const node = native.focusNode;
  const offset = native.focusOffset;
  let direction = "forward";
  if (type.endsWith("Backward")) {
    direction = "backward";
  }
  let granularity = "lineboundary";
  if (type.startsWith("deleteContent")) {
    granularity = "character";
  } else if (type.includes("Word")) {
    granularity = "word";
  } else if (type.includes("Hard")) {
    granularity = "paragraphboundary";
  }
  try {
    if (type === "deleteEntireSoftLine") {
      native.modify("move", "backward", "lineboundary");
    }
    native.modify("extend", direction, granularity);
    return read(native);
  } finally {
    native.collapse(node, offset);
  }
}

function pasteText(transfer: DataTransfer | null) {
  for (const type of ["text/markdown", "text/plain"]) {
    const text = transfer?.getData(type);
    if (text) {
      return text;
    }
  }
  return undefined;
}

function sameEndpoint(
  block: number,
  offset: number,
  otherBlock: number,
  otherOffset: number,
) {
  return block === otherBlock && offset === otherOffset;
}

function targetRange(
  range: StaticRange | undefined,
  before: DocumentSelection,
  read: ReadSelection,
) {
  const target = range && read(range);
  if (!target) {
    return null;
  }
  // getTargetRanges orders the endpoints and cannot encode affinity.
  const [anchor, start, focus, end] = target;
  if (
    (sameEndpoint(anchor, start, before[0], before[1]) &&
      sameEndpoint(focus, end, before[2], before[3])) ||
    (sameEndpoint(anchor, start, before[2], before[3]) &&
      sameEndpoint(focus, end, before[0], before[1]))
  ) {
    return before;
  }
  return target;
}

// oxlint-disable-next-line complexity -- Dispatch by Input Events inputType.
function inputEdit(
  event: InputEvent,
  before: DocumentSelection,
  read: ReadSelection,
): InputEdit | null {
  const [range] = event.getTargetRanges();
  const target = targetRange(range, before, read);
  const text = event.dataTransfer?.getData("text/plain") ?? event.data;
  switch (event.inputType) {
    case "insertFromPaste":
    case "insertFromPasteAsQuotation":
      return [
        EditCommand.Paste,
        before,
        pasteText(event.dataTransfer) ?? event.data ?? undefined,
        undefined,
        target ?? before,
      ];
    case "insertText":
    case "insertReplacementText":
    case "insertLink":
    case "insertFromYank":
    case "insertTranspose": {
      let typingAt;
      if (event.inputType === "insertText") {
        typingAt = Math.floor(performance.now());
      }
      return [
        EditCommand.ReplaceRange,
        before,
        text ?? undefined,
        typingAt,
        target ?? before,
      ];
    }
    case "deleteWordBackward":
    case "deleteWordForward":
    case "deleteSoftLineBackward":
    case "deleteSoftLineForward":
    case "deleteHardLineBackward":
    case "deleteHardLineForward":
    case "deleteEntireSoftLine": {
      const deletion = target ?? deletionTarget(event.inputType, before, read);
      return deletion && [EditCommand.ReplaceRange, before, "", undefined, deletion];
    }
    case "deleteByCut":
    case "deleteContent":
      return [EditCommand.ReplaceRange, before, "", undefined, target ?? before];
    case "deleteContentBackward":
    case "deleteContentForward": {
      const deletion = target ?? deletionTarget(event.inputType, before, read);
      if (
        deletion &&
        deletion[0] === deletion[2] &&
        deletion[0] === before[0] &&
        Math.abs(deletion[1] - deletion[3]) > 1
      ) {
        return [EditCommand.ReplaceRange, before, "", undefined, deletion];
      }
      if (event.inputType === "deleteContentBackward") {
        return [EditCommand.Backspace, before];
      }
      return [EditCommand.Delete, before];
    }
    case "insertParagraph":
      return [EditCommand.Enter, before];
    case "insertLineBreak":
      return [EditCommand.HardBreak, before];
    case "formatIndent":
      return [EditCommand.Indent, before];
    case "formatOutdent":
      return [EditCommand.Outdent, before];
    case "historyUndo":
      return [EditCommand.Undo, before];
    case "historyRedo":
      return [EditCommand.Redo, before];
    default:
      return null;
  }
}

function shortcut(event: KeyboardEvent) {
  if (event.ctrlKey || event.metaKey) {
    const key = event.key.toLowerCase();
    if (key === "z" && !event.shiftKey) {
      return EditCommand.Undo;
    }
    if (key === "z" || (key === "y" && !event.metaKey)) {
      return EditCommand.Redo;
    }
  } else if (event.key === "Tab") {
    if (event.shiftKey) {
      return EditCommand.Outdent;
    }
    return EditCommand.Indent;
  }
  return null;
}

export function attachEditor(
  editor: HTMLElement,
  markdown: Markdown,
  showError: (error: unknown) => void,
) {
  type State = ReturnType<Markdown["current"]>;
  let read: ReadSelection;
  let renderedSelection: DocumentSelection;
  let pending: { edit: InputEdit | null; before: DocumentSelection } | undefined;
  let composition:
    | { before: DocumentSelection; target?: DocumentSelection }
    | undefined;

  const restore = ({ nodes, selection }: State) => {
    renderedSelection = selection;
    read = renderEditableDocument(editor, nodes, selection);
  };
  const selectionNow = () => read(document.getSelection()) ?? renderedSelection;
  const restoreAt = (selection: DocumentSelection) =>
    restore({ ...markdown.current(), selection });
  const listeners = new AbortController();
  const listen = <EventName extends keyof HTMLElementEventMap>(
    type: EventName,
    listener: (event: HTMLElementEventMap[EventName]) => void,
  ) => editor.addEventListener(type, listener, { signal: listeners.signal });
  const previousEditable = editor.getAttribute("contenteditable");
  const edit = (...args: InputEdit) => {
    try {
      const result = markdown.edit(...args);
      if (result) {
        restore(result);
      }
      return Boolean(result);
    } catch (error) {
      showError(error);
      return false;
    }
  };

  const applyInput = (operation: InputEdit | null, event: InputEvent) => {
    if (!operation) {
      return false;
    }
    if (
      (operation[0] === EditCommand.ReplaceRange ||
        operation[0] === EditCommand.Paste) &&
      operation[2] === undefined
    ) {
      // Some platforms expose the replacement text only on the input event.
      let transfer = event.dataTransfer?.getData("text/plain");
      if (operation[0] === EditCommand.Paste) {
        transfer = pasteText(event.dataTransfer);
      }
      const text = transfer ?? event.data;
      if (text === null) {
        return false;
      }
      return edit(operation[0], operation[1], text, operation[3], operation[4]);
    }
    return edit(...operation);
  };

  listen("beforeinput", (event) => {
    if (composition || event.isComposing) {
      const [range] = event.getTargetRanges();
      if (composition && !composition.target && range) {
        composition.target = targetRange(range, composition.before, read) ?? undefined;
      }
      return;
    }
    pending = undefined;
    const before = selectionNow();
    const operation = inputEdit(event, before, read);
    if (event.cancelable) {
      event.preventDefault();
      applyInput(operation, event);
    } else {
      pending = { edit: operation, before };
    }
  });
  listen("input", (event) => {
    if (!(event instanceof InputEvent)) {
      return;
    }
    if (composition || event.isComposing) {
      return;
    }
    const before = pending?.before ?? renderedSelection;
    let operation;
    if (pending) {
      operation = pending.edit;
    } else {
      operation = inputEdit(event, before, read);
    }
    pending = undefined;
    if (!applyInput(operation, event)) {
      restoreAt(before);
    }
  });
  listen("compositionstart", () => {
    composition = { before: selectionNow() };
    pending = undefined;
  });
  listen("compositionend", (event) => {
    const current = composition;
    composition = undefined;
    if (
      current &&
      (!event.data ||
        !edit(
          EditCommand.ReplaceRange,
          current.before,
          event.data,
          undefined,
          current.target ?? current.before,
        ))
    ) {
      restoreAt(current.before);
    }
  });
  listen("keydown", (event) => {
    if (composition || event.isComposing || event.altKey) {
      return;
    }
    const command = shortcut(event);
    if (command !== null) {
      const applied = edit(command, selectionNow());
      if (applied || command === EditCommand.Undo || command === EditCommand.Redo) {
        event.preventDefault();
      }
    }
  });
  const writeSelection = (event: ClipboardEvent) => {
    const native = document.getSelection();
    if (!event.clipboardData || !native || native.isCollapsed) {
      return;
    }
    const range = read(native, true);
    if (!range) {
      return;
    }
    try {
      const text = markdown.selected(range);
      event.clipboardData.setData("text/plain", text);
      event.preventDefault();
      if (event.type === "cut") {
        const selection = read(native);
        if (selection) {
          edit(EditCommand.ReplaceRange, selection, "");
        }
      }
    } catch (error) {
      showError(error);
    }
  };
  listen("copy", writeSelection);
  listen("cut", writeSelection);
  listen("paste", (event) => {
    event.preventDefault();
    const text = pasteText(event.clipboardData);
    if (text) {
      edit(EditCommand.Paste, selectionNow(), text);
    }
  });
  listen("drop", (event) => event.preventDefault());
  listen("click", (event) => {
    if (event.target instanceof Element && event.target.closest("a")) {
      event.preventDefault();
    }
  });
  editor.setAttribute("contenteditable", "true");
  editor.focus();
  const destroy = () => {
    if (listeners.signal.aborted) {
      return;
    }
    listeners.abort();
    if (previousEditable === null) {
      editor.removeAttribute("contenteditable");
    } else {
      editor.setAttribute("contenteditable", previousEditable);
    }
  };
  try {
    restore(markdown.current());
  } catch (error) {
    destroy();
    throw error;
  }
  return { destroy };
}
