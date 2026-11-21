import { attachEditor } from "./editor.js";
import { loadMarkdown } from "./markdown.js";

const editor = document.querySelector("#editor");
const status = document.querySelector("#status");
if (!(editor instanceof HTMLElement) || !(status instanceof HTMLElement)) {
  throw new Error("エディターの要素が見つかりません");
}
try {
  attachEditor(editor, status, await loadMarkdown());
} catch (error) {
  status.textContent = `エディターを起動できませんでした（${String(error)}）`;
}
