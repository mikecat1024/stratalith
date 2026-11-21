import { attachEditor } from "./editor.js";
import { loadMarkdown } from "./markdown.js";

interface EditorOptions {
  wasmUrl: string | URL;
  onError?: (error: unknown) => void;
}

const mounted = new WeakSet<HTMLElement>();

/** Mount an empty Markdown editor. Destroy it before reusing the same element. */
export async function mountEditor(element: HTMLElement, options: EditorOptions) {
  if (mounted.has(element)) {
    throw new Error("An editor is already mounted on this element");
  }
  mounted.add(element);
  try {
    const markdown = await loadMarkdown(options.wasmUrl);
    const editor = attachEditor(element, markdown, options.onError ?? reportError);
    let active = true;
    return {
      destroy() {
        if (active) {
          active = false;
          editor.destroy();
          mounted.delete(element);
        }
      },
    };
  } catch (error) {
    mounted.delete(element);
    throw error;
  }
}
