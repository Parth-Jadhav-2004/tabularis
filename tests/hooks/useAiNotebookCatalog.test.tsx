import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAiNotebookCatalog } from "../../src/hooks/useAiNotebookCatalog";
import { listNotebooks, NOTEBOOKS_CHANGED_EVENT } from "../../src/utils/notebookStore";
import type { NotebookMetadata } from "../../src/types/notebook";

vi.mock("../../src/utils/notebookStore", () => ({ listNotebooks: vi.fn(), NOTEBOOKS_CHANGED_EVENT: "tabularis:notebooks-changed" }));

describe("notebook chat metadata", () => {
  beforeEach(() => { vi.mocked(listNotebooks).mockReset(); });

  it("reads metadata once per connection and refreshes after notebook deletion", async () => {
    vi.mocked(listNotebooks).mockResolvedValue([{ id: "book", title: "Revenue" }]);
    const fixture = renderHook(({ ids }) => useAiNotebookCatalog(ids), { initialProps: { ids: ["connection", "connection"] } });
    await waitFor(() => expect(fixture.result.current.connection).toHaveLength(1));
    fixture.rerender({ ids: ["connection"] });
    expect(listNotebooks).toHaveBeenCalledTimes(1);
    vi.mocked(listNotebooks).mockResolvedValue([]);
    act(() => { window.dispatchEvent(new Event(NOTEBOOKS_CHANGED_EVENT)); });
    await waitFor(() => expect(fixture.result.current.connection).toEqual([]));
  });

  it("does not infer deletion when a metadata read fails", async () => {
    vi.mocked(listNotebooks).mockRejectedValueOnce(new Error("unavailable"));
    const fixture = renderHook(() => useAiNotebookCatalog(["connection"]));
    await waitFor(() => expect(listNotebooks).toHaveBeenCalledOnce());
    expect(fixture.result.current.connection).toBeUndefined();
    vi.mocked(listNotebooks).mockResolvedValue([{ id: "book", title: "Recovered" }]);
    act(() => { window.dispatchEvent(new Event(NOTEBOOKS_CHANGED_EVENT)); });
    await waitFor(() => expect(fixture.result.current.connection?.[0].title).toBe("Recovered"));
    vi.mocked(listNotebooks).mockRejectedValue(new Error("unavailable again"));
    act(() => { window.dispatchEvent(new Event(NOTEBOOKS_CHANGED_EVENT)); });
    await waitFor(() => expect(fixture.result.current.connection).toBeUndefined());
  });

  it("ignores late reads from a previous connection", async () => {
    let resolve!: (items: NotebookMetadata[]) => void;
    vi.mocked(listNotebooks).mockReturnValueOnce(new Promise((done) => { resolve = done; }))
      .mockResolvedValue([{ id: "second", title: "Second" }]);
    const fixture = renderHook(({ id }) => useAiNotebookCatalog([id]), { initialProps: { id: "one" } });
    fixture.rerender({ id: "two" });
    await waitFor(() => expect(fixture.result.current.two?.[0].id).toBe("second"));
    await act(async () => { resolve([{ id: "first", title: "First" }]); });
    expect(fixture.result.current.one).toBeUndefined();
  });
});
