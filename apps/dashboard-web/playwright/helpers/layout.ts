import { expect, type Locator } from "@playwright/test";

type Box = { x: number; y: number; width: number; height: number };

async function box(locator: Locator): Promise<Box> {
  const bb = await locator.boundingBox();
  if (!bb) throw new Error("Unable to read bounding box (element not visible?)");
  return bb;
}

export async function expectNoHorizontalOverflow(
  locator: Locator,
  options?: { slackPx?: number; label?: string },
): Promise<void> {
  const slackPx = options?.slackPx ?? 1;
  const label = options?.label ?? "element";

  const dims = await locator.evaluate((el) => ({
    scrollWidth: el.scrollWidth,
    clientWidth: el.clientWidth,
  }));

  expect(
    dims.scrollWidth,
    `${label} scrollWidth=${dims.scrollWidth} should be <= clientWidth=${dims.clientWidth}`,
  ).toBeLessThanOrEqual(dims.clientWidth + slackPx);
}

export async function expectNoVerticalShiftDuring(
  action: () => Promise<void>,
  locator: Locator,
  options?: { tolerancePx?: number; label?: string },
): Promise<void> {
  const tolerancePx = options?.tolerancePx ?? 1;
  const label = options?.label ?? "element";

  const before = await box(locator);
  await action();
  const after = await box(locator);

  expect(
    Math.abs(after.y - before.y),
    `${label} y shifted: before=${before.y} after=${after.y}`,
  ).toBeLessThanOrEqual(tolerancePx);
  expect(
    Math.abs(after.height - before.height),
    `${label} height changed: before=${before.height} after=${after.height}`,
  ).toBeLessThanOrEqual(tolerancePx);
}

