import * as React from "react";

import { cn } from "@/lib/utils";

/** A native checkbox in the launcher's style. Pair it with a `Label` using `htmlFor`. */
function Checkbox({ className, ...props }: Omit<React.ComponentProps<"input">, "type">) {
    return (
        <input
            type="checkbox"
            data-slot="checkbox"
            className={cn(
                "size-4 shrink-0 rounded-[4px] border border-input accent-primary shadow-xs outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50",
                className,
            )}
            {...props}
        />
    );
}

export { Checkbox };
