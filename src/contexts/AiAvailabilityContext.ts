import { createContext } from "react";
import type { AiAvailability } from "../types/aiAvailability";

// Isolated surfaces fail closed until mounted inside the application provider.
export const AiAvailabilityContext = createContext<AiAvailability>({
  available: false, loading: false, providers: [], refresh: () => {},
});
