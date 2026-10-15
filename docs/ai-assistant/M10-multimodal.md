# M10. Multimodal input and rich output

**Depends on:** M2 (blobs), M4, M9 (charts should come from real query results) · **Size:** M

## Goal

Attach images and text files to a message; render charts and diagrams in answers.

## Design

### Images

- Composer: paste, drag and drop, or a file picker. Thumbnails in an attachments row, removable.
- Process in Rust (`image` crate, check whether it is already a dependency):
  - apply EXIF orientation
  - downscale to 2000px on the long edge
  - re-encode JPEG at ~0.9; keep PNG for screenshots
- Store as a sealed blob (M2) and reference it from an `Image` block.
- Limits: 4 per message, 5 MB each after processing.
- Shown only when `capabilities.images` is true for the selected model. With a model that doesn't support images, explain why the button is disabled; when switching to such a model in a conversation that has images, warn that earlier images will be dropped from the request.
- Sent to the provider as base64 (data URL for OpenAI-compatible, `image` source for Anthropic, `images` for Ollama).

### Text files

- Allowed: `.sql`, `.csv`, `.tsv`, `.txt`, `.md`, `.json`, `.yaml`, `.xml`, `.log`.
- Limits: 8 files, 48 KB each, 32k chars total in context. Truncation is shown on the chip.
- Encoding: UTF-8 first, then UTF-16 BOM; otherwise reject with a clear message.
- Stored as a sealed blob, sent as an untrusted-data layer (M6 layer 8).
- `.sql` files also get an "Open in editor" action.

### Rich output: block renderer registry

- `src/components/ai-chat/blocks/registry.ts`: maps fence languages to renderers (`sql` → `SqlBlock`, `chart` → `ChartBlock`, `mermaid` → `MermaidBlock`). Unknown languages fall back to a plain code block.
- **Charts** (`recharts`, already a dependency):
  - Keep the spec compatible with the notebook `CellChartConfig` (type, label column, value columns), so M8 can turn a chart into a notebook cell.
  - Fence ` ```chart ` with JSON `{ type: "bar" | "line" | "area" | "pie", title?, x, series: [{ key, label? }], data: [...] }`.
  - Validate the spec (zod or a hand-written guard); limits 8 series, 5k points, 256 KB. Invalid specs show the JSON and an error.
  - Theme colors from the design tokens, readable in light and dark.
  - Actions: copy data as CSV, save as PNG.
  - Prompt rule: chart data must come from a query the assistant ran in this conversation or from an attachment; never invent numbers.
- **Mermaid:**
  - Fence ` ```mermaid `, rendered with `mermaid` (new dependency, lazy-loaded) using `securityLevel: "strict"`, with no HTML labels and no click handlers.
  - Theme-aware.
  - On a parse error, show the source + error.
  - Base prompt hint: for ER questions, use `erDiagram` built from the real schema.

## Steps

- [ ] Attachment row in the composer + drag and drop + paste.
- [ ] Rust image processing + blob storage.
- [ ] Capability gating in the UI.
- [ ] Text file attachments + context layer.
- [ ] Renderer registry; migrate `SqlBlock` into it.
- [ ] Chart block + spec validation + prompt rule.
- [ ] Mermaid block (lazy import) + prompt hint.
- [ ] i18n.

## Tests

- Image pipeline: orientation, resize, size limit.
- File limits and encoding detection.
- Chart spec validation (valid, too many points, wrong types).
- Mermaid renderer rejects scripts/HTML labels (strict mode).

## Done when

- A screenshot of an ER diagram plus "create these tables" produces DDL.
- "Show monthly revenue as a chart" (Ask mode) runs a query and renders a chart from its result.
- An ER diagram for selected tables renders inline.

## Good first issue candidates

- Mermaid rendering, once the renderer registry exists.
