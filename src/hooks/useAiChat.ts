import { useContext } from "react";
import { AiChatContext } from "../contexts/AiChatContext";
import type { AiChatContextValue } from "../types/aiChat";

export function useAiChat(): AiChatContextValue {
  const context = useContext(AiChatContext);
  if (!context) throw new Error("useAiChat must be used within an AiChatProvider");
  return context;
}
