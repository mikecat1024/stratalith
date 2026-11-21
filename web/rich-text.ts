import type { DocumentSelection, MarkdownNode } from "./markdown.js";

const inlineBoundary = "\u200b";

const inlineFormatting = "b, strong, i, em, a, code";

function createInlineBoundary() {
  const boundary = document.createElement("span");
  boundary.dataset.inlineBoundary = "";
  boundary.textContent = inlineBoundary;
  return boundary;
}

function inlineBoundaryLength(text: Text): number {
  if (
    text.parentElement?.hasAttribute("data-inline-boundary") &&
    text.previousSibling === null &&
    text.data.startsWith(inlineBoundary)
  ) {
    return inlineBoundary.length;
  }
  return 0;
}

const containerTags = {
  paragraph: "p",
  quote: "blockquote",
  item: "li",
  strong: "strong",
  emphasis: "em",
} as const;

export function safeUrl(value: string, image = false): boolean {
  try {
    const { protocol } = new URL(value, document.baseURI);
    return (
      protocol === "https:" ||
      protocol === "http:" ||
      (!image && protocol === "mailto:")
    );
  } catch {
    return false;
  }
}

function containerElement(tag: string, children: MarkdownNode[]): HTMLElement {
  const result = document.createElement(tag);
  result.append(...children.map(renderNode));
  return result;
}

function imageDescription(nodes: MarkdownNode[]): string {
  return nodes
    .map((node) => {
      switch (node.kind) {
        case "text":
        case "code":
          return node.value;
        case "softBreak":
        case "hardBreak":
          return " ";
        default:
          return imageDescription(node.children);
      }
    })
    .join("");
}

function renderList(node: MarkdownNode & { kind: "ordered" | "bullet" }) {
  if (node.kind === "ordered") {
    const list = containerElement("ol", node.children);
    list.setAttribute("start", String(node.start));
    list.dataset.tight = String(node.tight);
    return list;
  }
  const list = containerElement("ul", node.children);
  list.dataset.tight = String(node.tight);
  return list;
}

// oxlint-disable-next-line complexity -- Each Markdown kind has a DOM representation.
function renderNode(node: MarkdownNode): Node {
  const { children } = node;
  switch (node.kind) {
    case "text":
      return document.createTextNode(node.value);
    case "heading":
      return containerElement(`h${node.level}`, children);
    case "paragraph":
    case "quote":
    case "item":
    case "strong":
    case "emphasis":
      return containerElement(containerTags[node.kind], children);
    case "bullet":
    case "ordered":
      return renderList(node);
    case "rule":
      return document.createElement("hr");
    case "codeBlock": {
      const pre = document.createElement("pre");
      pre.textContent = node.value;
      if (node.language !== null) {
        pre.dataset.language = node.language;
      }
      return pre;
    }
    case "code": {
      const code = document.createElement("code");
      code.textContent = node.value;
      return code;
    }
    case "htmlBlock":
    case "htmlInline": {
      // Raw CommonMark HTML stays in the document as literal, inert text.
      const raw = document.createElement(
        { htmlBlock: "pre", htmlInline: "code" }[node.kind],
      );
      raw.dataset.rawHtml = node.value;
      raw.contentEditable = "false";
      raw.textContent = node.value;
      return raw;
    }
    case "softBreak":
    case "hardBreak": {
      const line = document.createElement("br");
      line.dataset.markdownBreak = node.kind;
      return line;
    }
    case "link": {
      const link = containerElement("a", children);
      if (safeUrl(node.url)) {
        link.setAttribute("href", node.url);
      }
      if (node.title !== null) {
        link.title = node.title;
      }
      return link;
    }
    case "image": {
      const image = document.createElement("img");
      image.alt = imageDescription(children);
      if (safeUrl(node.url, true)) {
        image.setAttribute("src", node.url);
      }
      if (node.title !== null) {
        image.title = node.title;
      }
      return image;
    }
    default:
      throw new Error("Received invalid document data from Wasm");
  }
}

export function renderDocument(editor: HTMLElement, nodes: MarkdownNode[]) {
  editor.replaceChildren(...nodes.map(renderNode));
}

type Point = [Node, number];
type Endpoint = [number, number, number];

interface Span {
  start: number;
  end: number;
}

interface RenderedBlock {
  index: number;
  element: HTMLElement;
  length: number;
  copyLength: number;
  editable: boolean;
  spans: Map<Node, Span>;
}

function indexInline(node: Node, start: number, spans: Map<Node, Span>): number {
  let end = start;
  if (node instanceof Text) {
    end += Array.from(node.data.slice(inlineBoundaryLength(node))).length;
  } else if (
    node instanceof HTMLElement &&
    node.matches("img, [data-markdown-break], [data-raw-html]")
  ) {
    end++;
  } else {
    for (const child of node.childNodes) {
      end = indexInline(child, end, spans);
    }
  }
  spans.set(node, { start, end });
  return end;
}

function inlineOffset(block: RenderedBlock, node: Node, offset: number) {
  const span = block.spans.get(node);
  if (!span) {
    return 0;
  }
  if (node instanceof Text) {
    return (
      span.start +
      Array.from(node.data.slice(inlineBoundaryLength(node), offset)).length
    );
  }
  const next = node.childNodes[offset];
  return block.spans.get(next)?.start ?? span.end;
}

// oxlint-disable-next-line complexity -- Text offsets are UTF-16, atomic inlines use child indices.
function caretPoint(block: RenderedBlock, offset: number, upstream: boolean): Point {
  let last: Point = [block.element, 0];
  for (const [node, span] of block.spans) {
    if (offset < span.start || offset > span.end) {
      continue;
    }
    if (node instanceof Text) {
      const prefix = inlineBoundaryLength(node);
      const units = Array.from(node.data.slice(prefix))
        .slice(0, offset - span.start)
        .join("").length;
      last = [node, prefix + units];
      if (offset < span.end || upstream || prefix) {
        return last;
      }
    } else if (
      node instanceof HTMLElement &&
      node.matches("img, [data-markdown-break], [data-raw-html]") &&
      node.parentNode
    ) {
      const index = [...node.parentNode.childNodes].indexOf(node);
      last = [node.parentNode, index + Number(offset === span.end)];
      if (offset === span.start || upstream) {
        return last;
      }
    }
  }
  return last;
}

function pointAffinity(node: Node, offset: number) {
  if (node instanceof Text) {
    return Number(offset === node.length && offset > inlineBoundaryLength(node));
  }
  return Number(
    node instanceof HTMLElement &&
      node.matches(inlineFormatting) &&
      offset > 0 &&
      offset === node.childNodes.length,
  );
}

function prepareBlock(
  node: MarkdownNode,
  element: HTMLElement,
  index: number,
): RenderedBlock {
  if (node.kind === "paragraph" || node.kind === "heading") {
    if (!element.firstChild || element.lastChild instanceof HTMLBRElement) {
      element.append(document.createElement("br"));
    } else if (
      element.lastChild instanceof HTMLElement &&
      element.lastChild.matches(inlineFormatting)
    ) {
      element.append(createInlineBoundary());
    }
  }
  const editable = ["paragraph", "heading", "codeBlock"].includes(node.kind);
  const spans = new Map<Node, Span>();
  let length = 0;
  if (editable) {
    length = indexInline(element, 0, spans);
  }
  return {
    index,
    element,
    length,
    copyLength: length || Number(node.kind !== "paragraph"),
    editable,
    spans,
  };
}

function outerEndpoint(
  blocks: RenderedBlock[],
  point: Node,
  offset: number,
  copy: boolean,
  endEdge: boolean,
): Endpoint | null {
  const next = point.childNodes[offset];
  const prev = point.childNodes[offset - 1];
  const candidates = blocks.filter((block) => copy || block.editable);
  const right = candidates.find((block) => next?.contains(block.element));
  const left = candidates.filter((block) => prev?.contains(block.element)).at(-1);
  if (left && (endEdge || !right)) {
    if (copy) {
      return [left.index, left.copyLength, 0];
    }
    return [left.index, left.length, 0];
  }
  if (right) {
    return [right.index, 0, 0];
  }
  return null;
}

function samePoint(node: Node, offset: number, point: Point) {
  return node === point[0] && offset === point[1];
}

function nativePoints(selection: Selection | StaticRange) {
  if ("anchorNode" in selection) {
    const backward =
      selection.rangeCount > 0 &&
      !selection.isCollapsed &&
      selection.getRangeAt(0).endContainer === selection.anchorNode &&
      selection.getRangeAt(0).endOffset === selection.anchorOffset;
    return {
      anchor: selection.anchorNode,
      start: selection.anchorOffset,
      focus: selection.focusNode,
      end: selection.focusOffset,
      backward,
    };
  }
  return {
    anchor: selection.startContainer,
    start: selection.startOffset,
    focus: selection.endContainer,
    end: selection.endOffset,
    backward: false,
  };
}

/** Coordinates refer to this render only. A browser mutation never becomes an editor document. */
export function renderEditableDocument(
  editor: HTMLElement,
  nodes: MarkdownNode[],
  coordinates: DocumentSelection,
) {
  renderDocument(editor, nodes);
  const leaves: RenderedBlock[] = [];
  const blocks = new Map<Node, RenderedBlock>();
  const byIndex = new Map<number, RenderedBlock>();
  let nextIndex = 0;
  const visit = (node: MarkdownNode, element: Element) => {
    if (!(element instanceof HTMLElement)) {
      throw new Error("Missing rendered block");
    }
    const index = nextIndex++;
    if (
      ["quote", "ordered", "bullet", "item"].includes(node.kind) &&
      node.children.length
    ) {
      node.children.forEach((child, childIndex) =>
        visit(child, element.children[childIndex]),
      );
      return;
    }
    const block = prepareBlock(node, element, index);
    blocks.set(element, block);
    byIndex.set(index, block);
    leaves.push(block);
  };
  nodes.forEach((node, index) => visit(node, editor.children[index]));
  const blockAt = (index: number) => {
    const block = byIndex.get(index);
    if (!block) {
      throw new Error("Invalid selection block");
    }
    return block;
  };
  const [anchor, start, focus, end, affinity] = coordinates;
  const anchorPoint = caretPoint(blockAt(anchor), start, Boolean(affinity & 1));
  const focusPoint = caretPoint(blockAt(focus), end, Boolean(affinity & 2));
  document.getSelection()?.setBaseAndExtent(...anchorPoint, ...focusPoint);

  const endpoint = (
    point: Node,
    offset: number,
    copy: boolean,
    endEdge: boolean,
  ): Endpoint | null => {
    if (!editor.contains(point)) {
      return null;
    }
    for (let ancestor: Node | null = point; ancestor; ancestor = ancestor.parentNode) {
      const block = blocks.get(ancestor);
      if (block && (copy || block.editable)) {
        let position = inlineOffset(block, point, offset);
        if (copy && !block.length && endEdge && offset > 0) {
          position = block.copyLength;
        }
        return [block.index, position, pointAffinity(point, offset)];
      }
    }
    // Selecting an entire element places the DOM endpoint on its parent.
    return outerEndpoint(leaves, point, offset, copy, endEdge);
  };
  return (
    selection: Selection | StaticRange | null,
    copy = false,
  ): DocumentSelection | null => {
    if (!selection) {
      return null;
    }
    const points = nativePoints(selection);
    if (!points.anchor || !points.focus) {
      return null;
    }
    // DOM cannot distinguish both affinities at every caret (e.g. at block end).
    if (
      samePoint(points.anchor, points.start, anchorPoint) &&
      samePoint(points.focus, points.end, focusPoint)
    ) {
      return coordinates;
    }
    const from = endpoint(points.anchor, points.start, copy, points.backward);
    const to = endpoint(
      points.focus,
      points.end,
      copy,
      !points.backward &&
        !samePoint(points.anchor, points.start, [points.focus, points.end]),
    );
    if (!from || !to) {
      return null;
    }
    return [from[0], from[1], to[0], to[1], from[2] | (to[2] << 1)];
  };
}
