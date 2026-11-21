// Keep the tag order and field layout in sync with stratalith-wasm/src/tree.rs.
const kinds = [
  "paragraph",
  "quote",
  "bullet",
  "ordered",
  "item",
  "rule",
  "htmlBlock",
  "heading",
  "codeBlock",
  "text",
  "code",
  "softBreak",
  "hardBreak",
  "htmlInline",
  "strong",
  "emphasis",
  "link",
  "image",
] as const;

type Kind = (typeof kinds)[number];

export type MarkdownNode = { children: MarkdownNode[] } & (
  | {
      kind:
        | "paragraph"
        | "quote"
        | "item"
        | "rule"
        | "softBreak"
        | "hardBreak"
        | "strong"
        | "emphasis";
    }
  | { kind: "bullet"; tight: boolean }
  | { kind: "ordered"; tight: boolean; start: number }
  | { kind: "heading"; level: number }
  | { kind: "codeBlock"; language: string | null; value: string }
  | { kind: "htmlBlock" | "htmlInline" | "text" | "code"; value: string }
  | { kind: "link" | "image"; url: string; title: string | null }
);

const absent = 0xffffffff;
const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });

class TreeReader {
  private offset = 0;
  private readonly view: DataView;

  constructor(private readonly bytes: Uint8Array) {
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  }

  private byte() {
    return this.view.getUint8(this.offset++);
  }

  private number() {
    const value = this.view.getUint32(this.offset, true);
    this.offset += 4;
    return value;
  }

  private optionalString() {
    const length = this.number();
    if (length === absent) {
      return null;
    }
    const end = this.offset + length;
    const value = decoder.decode(this.bytes.subarray(this.offset, end));
    this.offset = end;
    return value;
  }

  private string() {
    const value = this.optionalString();
    if (value === null) {
      throw new Error("Wasm から不正な文書データを受け取りました");
    }
    return value;
  }

  // oxlint-disable-next-line complexity -- One case per wire layout.
  private node(kind: Kind): MarkdownNode {
    switch (kind) {
      case "bullet":
        return { kind, tight: this.byte() !== 0, children: this.nodes() };
      case "ordered":
        return {
          kind,
          start: this.number(),
          tight: this.byte() !== 0,
          children: this.nodes(),
        };
      case "heading":
        return { kind, level: this.byte() + 1, children: this.nodes() };
      case "codeBlock":
        return {
          kind,
          language: this.optionalString(),
          value: this.string(),
          children: this.nodes(),
        };
      case "htmlBlock":
      case "htmlInline":
      case "text":
      case "code":
        return { kind, value: this.string(), children: this.nodes() };
      case "link":
      case "image":
        return {
          kind,
          url: this.string(),
          title: this.optionalString(),
          children: this.nodes(),
        };
      default:
        return { kind, children: this.nodes() };
    }
  }

  nodes(): MarkdownNode[] {
    const count = this.number();
    const result: MarkdownNode[] = [];
    for (let index = 0; index < count; index++) {
      const kind = kinds[this.byte()];
      if (kind === undefined) {
        throw new Error("Wasm から不正な文書データを受け取りました");
      }
      result.push(this.node(kind));
    }
    return result;
  }
}

interface MarkdownExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory;
  prepare_markdown_input(length: number): number;
  selected_markdown(...selection: DocumentSelection): number;
  edit_rich_text(
    command: number,
    ...arguments_: [...DocumentSelection, number, ...DocumentSelection]
  ): number;
  rich_text_state(): number;
  markdown_output_pointer(): number;
  markdown_output_length(): number;
}

function isMarkdownApi(api: WebAssembly.Exports): api is MarkdownExports {
  return (
    api.memory instanceof WebAssembly.Memory &&
    typeof api.prepare_markdown_input === "function" &&
    typeof api.selected_markdown === "function" &&
    typeof api.edit_rich_text === "function" &&
    typeof api.rich_text_state === "function" &&
    typeof api.markdown_output_pointer === "function" &&
    typeof api.markdown_output_length === "function"
  );
}

/** Anchor and focus block indices and scalar offsets, followed by the upstream-affinity bit for each endpoint. */
export type DocumentSelection = [number, number, number, number, number];

export const enum EditCommand {
  InsertText = 0,
  Backspace = 1,
  Delete,
  Enter,
  Undo,
  Redo,
  HardBreak,
  Indent,
  Outdent,
  ReplaceRange,
  Paste,
}

function readEditorState(output: Uint8Array) {
  const view = new DataView(output.buffer, output.byteOffset, output.byteLength);
  const endpoint = (index: number) => view.getUint32(index * 4, true);
  const selection: DocumentSelection = [
    endpoint(0),
    endpoint(1),
    endpoint(2),
    endpoint(3),
    endpoint(4),
  ];
  return { nodes: new TreeReader(output.subarray(20)).nodes(), selection };
}

export async function loadMarkdown() {
  const response = await fetch("./stratalith.wasm");
  if (!response.ok) {
    throw new Error(response.statusText);
  }
  const { instance } = await WebAssembly.instantiate(await response.arrayBuffer(), {
    host: {
      update() {
        throw new Error("Unexpected plain-text editor patch");
      },
    },
  });
  const api = instance.exports;
  if (!isMarkdownApi(api)) {
    throw new Error("Wasm API を読み込めません");
  }

  const convert = (input: Uint8Array, run: () => number) => {
    const ptr = api.prepare_markdown_input(input.length);
    new Uint8Array(api.memory.buffer, ptr, input.length).set(input);
    const result = run();
    const output = new Uint8Array(
      api.memory.buffer,
      api.markdown_output_pointer(),
      api.markdown_output_length(),
    );
    if (!result) {
      throw new Error(decoder.decode(output));
    }
    return output;
  };

  return {
    current() {
      return readEditorState(convert(new Uint8Array(), () => api.rich_text_state()));
    },
    edit(
      command: EditCommand,
      selection: DocumentSelection,
      text?: string,
      typingAt?: number,
      target = selection,
    ) {
      const output = convert(encoder.encode(text), () =>
        api.edit_rich_text(command, ...selection, (typingAt ?? -1) + 1, ...target),
      );
      if (!output.length) {
        return null;
      }
      return readEditorState(output);
    },
    selected(selection: DocumentSelection): string {
      return decoder.decode(
        convert(new Uint8Array(), () => api.selected_markdown(...selection)),
      );
    },
  };
}
