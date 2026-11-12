import githubBlackUrl from "@/assets/brand/github-black.svg";
import githubWhiteUrl from "@/assets/brand/github-white.svg";
import googleUrl from "@/assets/brand/google.svg";
import { cn } from "@/lib/utils";

interface IconProps {
    className?: string;
}

/**
 * The GitHub mark. GitHub's guidelines ask for a dark mark on a light background and a light one
 * on a dark background, so both are rendered and the theme decides which one is visible.
 */
export function GitHubIcon({ className }: IconProps) {
    return (
        <>
            <img
                src={githubBlackUrl}
                alt=""
                aria-hidden="true"
                className={cn("size-4 dark:hidden", className)}
            />
            <img
                src={githubWhiteUrl}
                alt=""
                aria-hidden="true"
                className={cn("hidden size-4 dark:block", className)}
            />
        </>
    );
}

/** The Google "G". It is multicoloured and reads the same on both themes. */
export function GoogleIcon({ className }: IconProps) {
    return <img src={googleUrl} alt="" aria-hidden="true" className={cn("size-4", className)} />;
}
