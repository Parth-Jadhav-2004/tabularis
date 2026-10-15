import { useState, type ReactNode } from "react";
import { createAiEditorStore } from "../utils/aiEditor";
import { AiEditorContext } from "./AiEditorContext";

export function AiEditorProvider({ children }: { children: ReactNode }) {
  const [store] = useState(createAiEditorStore);
  return <AiEditorContext.Provider value={store}>{children}</AiEditorContext.Provider>;
}
