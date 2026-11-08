import { useContext } from "react";

import { UpdatesContext } from "@/lib/updates";

export function useUpdates() {
    const context = useContext(UpdatesContext);
    if (!context) {
        throw new Error("useUpdates must be used inside <UpdatesProvider>.");
    }
    return context;
}
