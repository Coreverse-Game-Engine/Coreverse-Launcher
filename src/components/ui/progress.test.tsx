import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { Progress } from "@/components/ui/progress";

describe("Progress", () => {
    it("exposes its value to assistive technology", () => {
        render(<Progress value={42} />);
        expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "42");
    });

    it("has no value while the total is unknown", () => {
        render(<Progress value={null} />);
        expect(screen.getByRole("progressbar")).not.toHaveAttribute("aria-valuenow");
    });
});
