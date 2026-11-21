import { attachEditor } from "./editor.js";
import { loadMarkdown } from "./markdown.js";

const editor = document.querySelector("#editor");
const status = document.querySelector("#status");
if (!(editor instanceof HTMLElement) || !(status instanceof HTMLElement)) {
  throw new Error("Missing editor elements");
}
try {
  attachEditor(editor, status, await loadMarkdown());
} catch (error) {
  status.textContent = `Could not start the editor: ${String(error)}`;
}
