import { attachEditor } from "../editor.js";
import { type DocumentSelection, EditCommand, loadMarkdown } from "../markdown.js";
import { renderEditableDocument } from "../rich-text.js";

type Markdown = Awaited<ReturnType<typeof loadMarkdown>>;
type State = ReturnType<Markdown["current"]>;

function equal(actual: unknown, expected: unknown) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(
      `Expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`,
    );
  }
}

function input(
  editor: HTMLElement,
  inputType: string,
  data: string | null = null,
  targetRanges: StaticRange[] = [],
) {
  editor.dispatchEvent(
    new InputEvent("beforeinput", { cancelable: true, inputType, data, targetRanges }),
  );
}

function clipboard(editor: HTMLElement, type: string, value = "") {
  const transfer = new DataTransfer();
  transfer.setData("text/plain", value);
  const event = new ClipboardEvent(type, { cancelable: true });
  // Firefox supplies a separate clipboard store for constructed events.
  Object.defineProperty(event, "clipboardData", { value: transfer });
  editor.dispatchEvent(event);
  return transfer.getData("text/plain");
}

function compose(editor: HTMLElement, value: string) {
  editor.dispatchEvent(new CompositionEvent("compositionstart"));
  editor.dispatchEvent(
    new InputEvent("input", {
      isComposing: true,
      inputType: "insertCompositionText",
      data: "仮",
    }),
  );
  editor.dispatchEvent(new CompositionEvent("compositionend", { data: value }));
  input(editor, "insertFromComposition", value);
}

function insert(markdown: Markdown, text: string) {
  markdown.edit(EditCommand.ReplaceRange, markdown.current().selection, text);
}

async function fixture(seed: (markdown: Markdown) => void) {
  const [host, direct] = await Promise.all([loadMarkdown(), loadMarkdown()]);
  seed(host);
  seed(direct);
  const editor = document.createElement("div");
  const status = document.createElement("p");
  document.body.replaceChildren(editor, status);
  attachEditor(editor, status, host);
  return { host, direct, editor };
}

function nodeAt(editor: HTMLElement, selector: string) {
  const text = editor.querySelector(selector)?.firstChild;
  if (!(text instanceof Text)) {
    throw new Error(`Missing text in ${selector}`);
  }
  return text;
}

function checkHistory(
  editor: HTMLElement,
  host: Markdown,
  direct: Markdown,
  before: DocumentSelection,
) {
  const edited = direct.current();
  equal(host.current(), edited);

  input(editor, "historyUndo");
  direct.edit(EditCommand.Undo, edited.selection);

  equal(host.current(), direct.current());
  equal(host.current().selection, before);

  input(editor, "historyRedo");
  direct.edit(EditCommand.Redo, before);

  equal(host.current(), edited);
  equal(direct.current(), edited);
}

async function replacement(dispatch: (editor: HTMLElement) => void) {
  const { host, direct, editor } = await fixture((markdown) =>
    insert(markdown, "a😀bc"),
  );
  const text = nodeAt(editor, "p");
  document.getSelection()?.setBaseAndExtent(text, 4, text, 1);
  const before: DocumentSelection = [0, 3, 0, 1, 0];

  dispatch(editor);
  direct.edit(EditCommand.ReplaceRange, before, "日");

  equal(editor.textContent, "a日c");
  checkHistory(editor, host, direct, before);
}

async function deletion(type: string) {
  const { host, direct, editor } = await fixture((markdown) =>
    insert(markdown, "one two"),
  );
  const text = nodeAt(editor, "p");
  const before = direct.current().selection;
  const target: DocumentSelection = [0, 4, 0, 7, 2];
  const range = new StaticRange({
    startContainer: text,
    startOffset: 4,
    endContainer: text,
    endOffset: 7,
  });

  input(editor, type, null, [range]);
  direct.edit(EditCommand.ReplaceRange, before, "", undefined, target);

  equal(editor.textContent, "one ");
  checkHistory(editor, host, direct, before);
}

async function backspace() {
  const { host, direct, editor } = await fixture((markdown) => {
    insert(markdown, "-");
    insert(markdown, " ");
    insert(markdown, "item");
  });
  document.getSelection()?.collapse(nodeAt(editor, "li p"), 0);
  const before: DocumentSelection = [2, 0, 2, 0, 0];

  input(editor, "deleteContentBackward");
  direct.edit(EditCommand.Backspace, before);

  equal(editor.innerHTML, "<p>item</p>");
  checkHistory(editor, host, direct, before);
}

async function cancellation() {
  const { host, direct, editor } = await fixture((markdown) => {
    insert(markdown, "a");
    insert(markdown, "b");
    markdown.edit(EditCommand.Undo, markdown.current().selection);
  });
  const before = direct.current();

  compose(editor, "");

  equal(host.current(), before);

  input(editor, "historyRedo");
  direct.edit(EditCommand.Redo, before.selection);

  equal(host.current(), direct.current());
  equal(editor.textContent, "ab");
}

async function copy(
  seed: (markdown: Markdown) => void,
  select: (editor: HTMLElement) => void,
  range: DocumentSelection,
  expected: string,
) {
  const { direct, editor } = await fixture(seed);
  select(editor);

  const copied = clipboard(editor, "copy");

  equal(copied, direct.selected(range));
  equal(copied, expected);
}

function coordinates(
  selection: DocumentSelection,
  focusParent: string,
  focusOffset: number,
) {
  const editor = document.createElement("div");
  editor.contentEditable = "true";
  document.body.replaceChildren(editor);
  const nodes: State["nodes"] = [
    {
      kind: "paragraph",
      children: [
        { kind: "text", value: "😀", children: [] },
        { kind: "strong", children: [{ kind: "text", value: "x", children: [] }] },
      ],
    },
    { kind: "paragraph", children: [] },
  ];

  const read = renderEditableDocument(editor, nodes, selection);

  const native = document.getSelection();
  equal(native?.focusNode?.parentElement?.tagName, focusParent);
  equal(native?.focusOffset, focusOffset);
  equal(read(native), selection);
}

async function nonCancelableReplacement() {
  const { host, direct, editor } = await fixture((markdown) =>
    insert(markdown, "old word"),
  );
  const before = direct.current().selection;
  const text = nodeAt(editor, "p");
  const range = new StaticRange({
    startContainer: text,
    startOffset: 0,
    endContainer: text,
    endOffset: 3,
  });
  const init = {
    inputType: "insertReplacementText",
    data: "new",
    targetRanges: [range],
  };

  editor.dispatchEvent(new InputEvent("beforeinput", init));
  editor.replaceChildren("tentative native DOM");
  editor.dispatchEvent(new InputEvent("input", init));
  direct.edit(EditCommand.ReplaceRange, before, "new", undefined, [0, 0, 0, 3, 0]);

  equal(editor.textContent, "new word");
  checkHistory(editor, host, direct, before);
}

async function structuredPaste(dispatch: (editor: HTMLElement, text: string) => void) {
  const { host, editor } = await fixture((markdown) => insert(markdown, "前😀後"));
  const text = nodeAt(editor, "p");
  document.getSelection()?.setBaseAndExtent(text, 3, text, 1);

  dispatch(editor, "## 見出し\n\n- **項目**");

  equal(
    Array.from(editor.children, (element) => element.tagName),
    ["P", "H2", "UL", "P"],
  );
  equal(editor.querySelector("h2")?.textContent, "見出し");
  equal(editor.querySelector("li strong")?.textContent, "項目");
  equal(editor.firstElementChild?.textContent, "前");
  equal(editor.lastElementChild?.textContent, "後");
  const pasted = host.current();

  input(editor, "historyUndo");

  equal(editor.textContent, "前😀後");
  equal(host.current().selection, [0, 2, 0, 1, 0]);

  input(editor, "historyRedo");

  equal(host.current(), pasted);
}

async function emojiDeletion(
  type: string,
  caret: number,
  ranges: (range: StaticRange) => StaticRange[] = (range) => [range],
) {
  const { host, editor } = await fixture((markdown) => insert(markdown, "a👍🏽b"));
  const text = nodeAt(editor, "p");
  document.getSelection()?.collapse(text, caret);
  const range = new StaticRange({
    startContainer: text,
    startOffset: 1,
    endContainer: text,
    endOffset: 5,
  });

  input(editor, type, null, ranges(range));

  equal(editor.textContent, "ab");

  input(editor, "historyUndo");

  equal(editor.textContent, "a👍🏽b");
  equal(host.current().selection[1], Array.from(text.data.slice(0, caret)).length);
}

async function formattedCut() {
  const { editor } = await fixture((markdown) =>
    markdown.edit(EditCommand.Paste, markdown.current().selection, "a **bold** b"),
  );
  const bold = nodeAt(editor, "strong");
  document.getSelection()?.setBaseAndExtent(bold, 4, bold, 0);

  const cut = clipboard(editor, "cut");

  equal(cut, "**bold**");
  equal(editor.textContent, "a  b");

  input(editor, "historyUndo");

  equal(editor.querySelector("strong")?.textContent, "bold");
  equal(document.getSelection()?.toString(), "bold");
  equal(document.getSelection()?.anchorOffset, 4);

  input(editor, "historyRedo");
  clipboard(editor, "paste", cut);

  equal(editor.querySelector("strong")?.textContent, "bold");
}

async function run() {
  await emojiDeletion("deleteContentBackward", 5);
  await emojiDeletion("deleteContentForward", 1);
  await emojiDeletion("deleteContentBackward", 5, () => []);
  await formattedCut();
  await structuredPaste((editor, text) => clipboard(editor, "paste", text));
  await structuredPaste((editor, text) => input(editor, "insertFromPaste", text));
  await nonCancelableReplacement();

  await replacement((editor) => input(editor, "insertText", "日"));
  await replacement((editor) => clipboard(editor, "paste", "日"));
  await replacement((editor) => compose(editor, "日"));
  await deletion("deleteWordBackward");
  await deletion("deleteSoftLineBackward");
  await backspace();
  await cancellation();
  await copy(
    (markdown) => {
      insert(markdown, "#");
      insert(markdown, " ");
    },
    (editor) => document.getSelection()?.setBaseAndExtent(editor, 0, editor, 1),
    [0, 0, 0, 1, 0],
    "# ",
  );
  await copy(
    (markdown) => {
      insert(markdown, "line");
      markdown.edit(EditCommand.HardBreak, markdown.current().selection);
    },
    (editor) => document.getSelection()?.setBaseAndExtent(editor, 0, editor, 1),
    [0, 0, 0, 5, 0],
    "line\\\n",
  );
  await copy(
    (markdown) => {
      insert(markdown, "abc");
      markdown.edit(EditCommand.Enter, markdown.current().selection);
      insert(markdown, "def");
    },
    (editor) =>
      document
        .getSelection()
        ?.setBaseAndExtent(nodeAt(editor, "p"), 1, nodeAt(editor, "p:last-child"), 0),
    [0, 1, 1, 0, 0],
    "bc\n\n",
  );
  await copy(
    (markdown) => {
      insert(markdown, "-");
      insert(markdown, " ");
      insert(markdown, "outer");
      markdown.edit(EditCommand.Enter, markdown.current().selection);
      insert(markdown, "inner");
      markdown.edit(EditCommand.Indent, markdown.current().selection);
    },
    (editor) =>
      document
        .getSelection()
        ?.setBaseAndExtent(nodeAt(editor, "li li p"), 5, nodeAt(editor, "li p"), 0),
    [5, 5, 2, 0, 1],
    "- outer\n  - inner",
  );
  coordinates([1, 0, 0, 1, 2], "P", 2);
  coordinates([0, 1, 0, 1, 0], "STRONG", 0);
  coordinates([0, 1, 0, 1, 3], "P", 2);
}

try {
  await run();
  document.body.textContent = "PASS";
} catch (error) {
  document.body.textContent = String(error);
  throw error;
}
