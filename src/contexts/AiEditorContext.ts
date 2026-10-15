import { createContext } from "react";
import type { AiEditorStore } from "../types/aiEditor";

export const AiEditorContext = createContext<AiEditorStore | null>(null);
