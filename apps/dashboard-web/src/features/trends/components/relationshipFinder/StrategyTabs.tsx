"use client";

import clsx from "clsx";
import type { KeyboardEvent } from "react";
import { Card } from "@/components/ui/card";
import type { Strategy } from "../../types/relationshipFinder";
import { STRATEGY_LABELS } from "../../types/relationshipFinder";

const STRATEGIES: Strategy[] = [
  "unified",
  "similarity",
  "correlation",
  "events",
  "cooccurrence",
];

type StrategyTabsProps = {
  selected: Strategy;
  onChange: (strategy: Strategy) => void;
  disabled?: boolean;
};

export default function StrategyTabs({
  selected,
  onChange,
  disabled = false,
}: StrategyTabsProps) {
  const handleKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (disabled) return;

    const currentIndex = STRATEGIES.indexOf(selected);
    let nextIndex = currentIndex;

    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
        e.preventDefault();
        nextIndex = (currentIndex + 1) % STRATEGIES.length;
        break;
      case "ArrowLeft":
      case "ArrowUp":
        e.preventDefault();
        nextIndex = (currentIndex - 1 + STRATEGIES.length) % STRATEGIES.length;
        break;
      case "Home":
        e.preventDefault();
        nextIndex = 0;
        break;
      case "End":
        e.preventDefault();
        nextIndex = STRATEGIES.length - 1;
        break;
      case "1":
      case "2":
      case "3":
      case "4":
      case "5":
        e.preventDefault();
        nextIndex = parseInt(e.key, 10) - 1;
        break;
      default:
        return;
    }

    if (nextIndex !== currentIndex) {
      onChange(STRATEGIES[nextIndex]!);
    }
  };

  return (
    <Card
      role="tablist"
      aria-label="Analysis strategy"
      className="inline-flex flex-row gap-0 overflow-hidden rounded-lg p-0 shadow-sm"
      onKeyDown={handleKeyDown}
    >
      {STRATEGIES.map((strategy, index) => {
        const isSelected = strategy === selected;
        return (
          <button
            key={strategy}
            role="tab"
            type="button"
            id={`strategy-tab-${strategy}`}
            aria-selected={isSelected}
            aria-controls={`strategy-panel-${strategy}`}
            tabIndex={isSelected ? 0 : -1}
            disabled={disabled}
            onClick={() => onChange(strategy)}
            className={clsx(
              "relative px-3 py-2 text-xs font-semibold transition-colors focus:z-10 focus:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-indigo-500",
              isSelected
                ? "bg-indigo-600 text-white"
 : "bg-white text-foreground hover:bg-muted",
              disabled && "cursor-not-allowed opacity-50",
              index > 0 && "border-l border-border",
            )}
            title={`${STRATEGY_LABELS[strategy]} [${index + 1}]`}
          >
            {STRATEGY_LABELS[strategy]}
          </button>
        );
      })}
    </Card>
  );
}
