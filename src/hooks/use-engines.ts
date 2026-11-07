import { useContext } from "react";

import { EnginesContext } from "@/lib/engines";

export function useEngines() {
    const context = useContext(EnginesContext);
    if (!context) {
        throw new Error("useEngines must be used inside <EnginesProvider>.");
    }
    return context;
}
