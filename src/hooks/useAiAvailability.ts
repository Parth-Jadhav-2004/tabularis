import { useContext } from "react";
import { AiAvailabilityContext } from "../contexts/AiAvailabilityContext";

export function useAiAvailability() {
  return useContext(AiAvailabilityContext);
}
