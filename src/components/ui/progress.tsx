import { cn } from "@/lib/utils";

/** A progress bar. `value` is a whole percent, or `null` when the total is not known yet. */
function Progress({ value, className }: { value: number | null; className?: string }) {
    return (
        <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={value ?? undefined}
            className={cn("h-2 w-full overflow-hidden rounded-full bg-secondary", className)}
        >
            <div
                className={cn(
                    "h-full rounded-full bg-primary transition-[width]",
                    value === null && "w-1/3 animate-pulse",
                )}
                style={value === null ? undefined : { width: `${value}%` }}
            />
        </div>
    );
}

export { Progress };
