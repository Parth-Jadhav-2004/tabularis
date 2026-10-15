import { createContext } from "react";
import type { AiChatContextValue } from "../types/aiChat";

export const AiChatContext = createContext<AiChatContextValue | undefined>(undefined);
