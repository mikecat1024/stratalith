import { attachEditor } from "../editor.js";
import { EditCommand, loadMarkdown } from "../markdown.js";

interface Trace {
  startContent: string;
  endContent: string;
  txns: { patches: [number, number, string][] }[];
}

function countOption(name: string, fallback: number, max: number) {
  const count = Number(new URLSearchParams(location.search).get(name) ?? fallback);
  if (!Number.isInteger(count) || count < 0 || count > max) {
    throw new Error(`${name} must be an integer between 0 and ${max}`);
  }
  return count;
}

function element(selector: string) {
  const found = document.querySelector<HTMLElement>(selector);
  if (!found) {
    throw new Error("Missing " + selector);
  }
  return found;
}

async function replay() {
  const editor = element("#editor");
  const status = element("#status");
  // run.sh verifies the hash of the pinned upstream fixture.
  // oxlint-disable-next-line typescript/no-unsafe-assignment -- SHA-256 verified fixture.
  const trace: Trace = await (await fetch("./seph-blog1.json")).json();
  const transactions = countOption("transactions", 250, trace.txns.length);
  if (!transactions) {
    throw new Error("Select at least one transaction");
  }
  const paragraphs = countOption("paragraphs", 1000, 10000);
  const split = trace.txns.length - transactions;
  let initial = trace.startContent;
  for (const txn of trace.txns.slice(0, split)) {
    for (const [pos, removed, inserted] of txn.patches) {
      initial = initial.slice(0, pos) + inserted + initial.slice(pos + removed);
    }
  }

  const markdown = await loadMarkdown();
  const prefix = "Benchmark paragraph.\n\n".repeat(paragraphs);
  markdown.edit(
    EditCommand.Paste,
    markdown.current().selection,
    prefix + "~~~\nseed\n~~~",
  );
  attachEditor(editor, status, markdown);
  const replace = (pos: number, removed: number, inserted: string) => {
    const block = editor.querySelector("pre");
    if (!block) {
      throw new Error("Missing trace text");
    }
    const text = block.firstChild ?? block;
    document.getSelection()?.setBaseAndExtent(text, pos, text, pos + removed);
    const event = new InputEvent("beforeinput", {
      cancelable: true,
      inputType: "insertText",
      data: inserted,
    });
    const start = performance.now();
    editor.dispatchEvent(event);
    void editor.offsetHeight;
    return performance.now() - start;
  };
  replace(0, element("#editor pre").textContent.length, initial);
  if (element("#editor pre").textContent !== initial) {
    throw new Error("Invalid starting state");
  }
  const samples: number[] = [];
  for (const txn of trace.txns.slice(split)) {
    for (const [pos, removed, inserted] of txn.patches) {
      samples.push(replace(pos, removed, inserted));
    }
    if (samples.length % 25 === 0) {
      await new Promise((resolve) => setTimeout(resolve, 0));
    }
  }
  if (element("#editor pre").textContent !== trace.endContent) {
    throw new Error("Trace result mismatch");
  }
  samples.sort((left, right) => left - right);
  const percentile = (fraction: number) =>
    samples[Math.ceil(samples.length * fraction) - 1];
  return {
    trace: "seph-blog1 (ASCII)",
    paragraphs,
    transactions,
    patches: samples.length,
    metric: "beforeinput through synchronous layout (ms)",
    p50: percentile(0.5),
    p95: percentile(0.95),
    max: samples.at(-1),
    browser: navigator.userAgent,
  };
}

const report = document.createElement("pre");
document.body.prepend(report);
report.textContent = "Running editing trace…";
try {
  report.textContent = JSON.stringify(await replay(), null, 2);
} catch (error) {
  report.textContent = String(error);
  throw error;
}
