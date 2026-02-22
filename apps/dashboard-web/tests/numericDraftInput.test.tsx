import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import { NumericDraftInput } from "@/components/forms/NumericDraftInput";

function DraftHarness({
  initialValue,
  min,
  max,
  enforceRange,
  clampOnBlur,
  integer,
  emptyBehavior,
}: {
  initialValue: number;
  min?: number;
  max?: number;
  enforceRange?: boolean;
  clampOnBlur?: boolean;
  integer?: boolean;
  emptyBehavior?: "set" | "keep";
}) {
  const [value, setValue] = useState<number>(initialValue);
  return (
    <div>
      <NumericDraftInput
        aria-label="numeric"
        value={value}
        onValueChange={(next) => {
          if (typeof next === "number") {
            setValue(next);
          }
        }}
        min={min}
        max={max}
        enforceRange={Boolean(enforceRange)}
        clampOnBlur={Boolean(clampOnBlur)}
        integer={Boolean(integer)}
        emptyBehavior={emptyBehavior}
      />
      <div data-testid="committed">{String(value)}</div>
    </div>
  );
}

describe("NumericDraftInput", () => {
  it("lets users type decimals without losing the dot mid-edit", async () => {
    const user = userEvent.setup();
    render(<DraftHarness initialValue={0} />);

    const input = screen.getByRole("textbox", { name: "numeric" });
    await user.click(input);
    await user.clear(input);

    await user.type(input, "0.");
    expect(input).toHaveValue("0.");

    await user.type(input, "25");
    expect(input).toHaveValue("0.25");

    await user.tab();
    expect(input).toHaveValue("0.25");
    expect(screen.getByTestId("committed")).toHaveTextContent("0.25");
  });

  it("does not block intermediate typing when range enforcement is enabled", async () => {
    const user = userEvent.setup();
    render(
      <DraftHarness
        initialValue={10}
        min={10}
        max={15}
        enforceRange
        clampOnBlur
        integer
        emptyBehavior="keep"
      />,
    );

    const input = screen.getByRole("textbox", { name: "numeric" });
    await user.click(input);
    await user.clear(input);

    await user.type(input, "1");
    expect(input).toHaveValue("1");
    expect(screen.getByTestId("committed")).toHaveTextContent("10");

    await user.type(input, "2");
    expect(input).toHaveValue("12");
    expect(screen.getByTestId("committed")).toHaveTextContent("12");
  });

  it("supports typing a leading '-' without committing NaN", async () => {
    const user = userEvent.setup();
    render(
      <DraftHarness
        initialValue={0}
        min={-180}
        max={180}
        enforceRange
        clampOnBlur
        integer
        emptyBehavior="keep"
      />,
    );

    const input = screen.getByRole("textbox", { name: "numeric" });
    await user.click(input);
    await user.clear(input);

    await user.type(input, "-");
    expect(input).toHaveValue("-");
    expect(screen.getByTestId("committed")).toHaveTextContent("0");

    await user.type(input, "1");
    expect(input).toHaveValue("-1");
    expect(screen.getByTestId("committed")).toHaveTextContent("-1");
  });
});

