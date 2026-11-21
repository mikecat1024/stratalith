import { attachEditor } from "../../packages/browser/src/editor.js";
import {
  type DocumentSelection,
  EditCommand,
  type MarkdownNode,
  loadMarkdown,
} from "../../packages/browser/src/markdown.js";

const sample = `# Field notes

> Take the scenic route.

Leave room for **one more stop**.

- Explore the coast
  - Walk the headland
- Pack a camera`;

const editor = document.querySelector("#editor");
const source = document.querySelector("#source");
const status = document.querySelector("#status");
if (
  !(editor instanceof HTMLElement) ||
  !(source instanceof HTMLElement) ||
  !(status instanceof HTMLElement)
) {
  throw new Error("Missing editor elements");
}

function showError(error: unknown) {
  if (status instanceof HTMLElement) {
    status.textContent = String(error);
    status.hidden = false;
  }
}

function inlineLength(node: MarkdownNode): number {
  switch (node.kind) {
    case "text":
    case "code":
      return Array.from(node.value).length;
    case "link":
    case "strong":
    case "emphasis":
      return node.children.reduce((sum, child) => sum + inlineLength(child), 0);
    default:
      return 1;
  }
}

// The clipboard API addresses blocks in preorder and text by Unicode scalar offset.
function documentRange(nodes: MarkdownNode[]): DocumentSelection {
  let index = 0;
  let first = -1;
  let last = 0;
  let end = 0;
  const visit = (node: MarkdownNode) => {
    const block = index++;
    if (
      ["quote", "item", "ordered", "bullet"].includes(node.kind) &&
      node.children.length
    ) {
      node.children.forEach(visit);
      return;
    }
    if (first < 0) {
      first = block;
    }
    last = block;
    if (node.kind === "codeBlock") {
      end = Array.from(node.value).length || 1;
    } else if (node.children.length) {
      end = node.children.reduce((sum, child) => sum + inlineLength(child), 0);
    } else {
      end = Number(node.kind !== "paragraph");
    }
  };
  nodes.forEach(visit);
  return [first, 0, last, end, 0];
}

try {
  const markdown = await loadMarkdown("./stratalith.wasm");
  const demo = {
    ...markdown,
    edit(...args: Parameters<typeof markdown.edit>) {
      const state = markdown.edit(...args);
      if (state) {
        try {
          source.textContent = markdown.selected(documentRange(state.nodes));
        } catch (error) {
          source.textContent = String(error);
        }
      }
      return state;
    },
  };
  demo.edit(EditCommand.Paste, demo.current().selection, sample);
  attachEditor(editor, demo, showError);
  status.hidden = true;
} catch (error) {
  showError(error);
}
