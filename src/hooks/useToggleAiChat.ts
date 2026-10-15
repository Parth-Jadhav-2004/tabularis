import { useCallback, useContext } from "react";
import { RightSidebarContext } from "../contexts/RightSidebarContext";
import { useAiAvailability } from "./useAiAvailability";

export function useToggleAiChat() {
  const sidebar = useContext(RightSidebarContext);
  const { available } = useAiAvailability();
  return useCallback(() => {
    if (!sidebar || !available) return;
    if (sidebar.isOpen && sidebar.activePanel === "ai-chat") sidebar.close();
    else {
      sidebar.setActivePanel("ai-chat");
      if (!sidebar.isOpen) sidebar.toggle();
    }
  }, [available, sidebar]);
}
