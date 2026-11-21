import { mountEditor } from "../../packages/browser/src/index.js";

const editor = document.querySelector("#editor");
const status = document.querySelector("#status");
if (!(editor instanceof HTMLElement) || !(status instanceof HTMLElement)) {
  throw new Error("Missing editor elements");
}
try {
  await mountEditor(editor, {
    wasmUrl: "./stratalith.wasm",
    onError(error) {
      status.textContent = String(error);
      status.hidden = false;
    },
  });
  status.hidden = true;
} catch (error) {
  status.textContent = `Could not start the editor: ${String(error)}`;
}
