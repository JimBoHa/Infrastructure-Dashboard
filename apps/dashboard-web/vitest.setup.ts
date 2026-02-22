import '@testing-library/jest-dom/vitest';
import { vi } from "vitest";

const noop = () => {};

const css = (globalThis as unknown as { CSS?: unknown }).CSS;
if (!css || typeof css !== "object" || typeof (css as { supports?: unknown }).supports !== "function") {
  Object.defineProperty(globalThis, "CSS", {
    value: {
      ...(css && typeof css === "object" ? css : {}),
      supports: () => false,
    },
    configurable: true,
  });
}

const createCanvasContext = (canvas: HTMLCanvasElement) =>
  ({
    canvas,
    fillRect: noop,
    clearRect: noop,
    getImageData: () => ({ data: [] }),
    putImageData: noop,
    createImageData: () => ({ data: [] }),
    setTransform: noop,
    resetTransform: noop,
    drawImage: noop,
    save: noop,
    fillText: noop,
    restore: noop,
    beginPath: noop,
    moveTo: noop,
    lineTo: noop,
    closePath: noop,
    stroke: noop,
    translate: noop,
    scale: noop,
    rotate: noop,
    arc: noop,
    fill: noop,
    measureText: () => ({ width: 0 }),
    transform: noop,
    rect: noop,
    clip: noop,
  }) as unknown as CanvasRenderingContext2D;

Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
  value(this: HTMLCanvasElement) {
    return createCanvasContext(this);
  },
});

Object.defineProperty(HTMLAnchorElement.prototype, "click", {
  value: vi.fn(),
});

if (!("ResizeObserver" in globalThis)) {
  class ResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
  }

  Object.defineProperty(globalThis, "ResizeObserver", { value: ResizeObserver });
}

if (!("matchMedia" in globalThis) || typeof globalThis.matchMedia !== "function") {
  Object.defineProperty(globalThis, "matchMedia", {
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: noop, // deprecated
      removeListener: noop, // deprecated
      addEventListener: noop,
      removeEventListener: noop,
      dispatchEvent: () => false,
    }),
    configurable: true,
  });
}
