import type { AiMessage } from "../types/aiChat";

export const AI_NOTEBOOK_REFERENCE_PREFIX = "#tabularis-cell-";

// Resolve the indexes seen by this answer, rather than the notebook's current
// order. The ID mapping is bounded metadata in the preceding user's context.
export function aiNotebookMessageReferences(messages: readonly AiMessage[], assistantIndex: number,
  notebookId: string, availableIds: ReadonlySet<string>): Map<number, string> {
  const result = new Map<number, string>();
  let user: AiMessage | undefined;
  for (let index = assistantIndex - 1; index >= 0; index--) {
    if (messages[index].role === "user") { user = messages[index]; break; }
  }
  if (!user) return result;
  const items = user.blocks.flatMap((block) => block.type === "context" ? block.items : []);
  if (!items.some((item) => item.kind === "notebook" && item.id === notebookId)) return result;
  for (const item of items) {
    if (item.kind !== "notebook_cell_ref" && item.kind !== "notebook_cell") continue;
    const match = /^#([1-9]\d{0,5})(?:$| · )/u.exec(item.label);
    if (!match || !availableIds.has(item.id)) continue;
    const index = Number(match[1]);
    if (!result.has(index)) result.set(index, item.id);
  }
  return result;
}

interface MarkdownNode {
  type: string;
  value?: string;
  url?: string;
  children?: MarkdownNode[];
}

// Transform prose only. Code, existing links, images and HTML retain their
// normal Markdown treatment and cannot acquire notebook navigation handlers.
export function aiNotebookReferencePlugin(cells: ReadonlyMap<number, string>) {
  return () => (tree: MarkdownNode) => {
    const visit = (node: MarkdownNode) => {
      if (!node.children || ["code", "inlineCode", "html", "link", "linkReference", "image", "imageReference", "definition"].includes(node.type)) return;
      node.children = node.children.flatMap((child): MarkdownNode[] => {
        if (child.type !== "text" || !child.value) { visit(child); return [child]; }
        const text = child.value;
        const pattern = /(^|[^\p{L}\p{N}_#])#([1-9]\d{0,5})(?![\p{L}\p{N}_])/gu;
        const parts: MarkdownNode[] = [];
        let cursor = 0;
        for (const match of text.matchAll(pattern)) {
          const index = Number(match[2]);
          if (!cells.has(index)) continue;
          const start = match.index + match[1].length;
          if (start > cursor) parts.push({ type: "text", value: text.slice(cursor, start) });
          parts.push({ type: "link", url: `${AI_NOTEBOOK_REFERENCE_PREFIX}${index}`, children: [{ type: "text", value: `#${index}` }] });
          cursor = start + match[2].length + 1;
        }
        if (cursor === 0) return [child];
        if (cursor < text.length) parts.push({ type: "text", value: text.slice(cursor) });
        return parts;
      });
    };
    visit(tree);
  };
}
