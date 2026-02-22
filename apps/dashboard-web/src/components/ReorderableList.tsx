"use client";

import clsx from "clsx";
import { useCallback, useRef, type ReactNode } from "react";
import { useDrag, useDrop } from "react-dnd";
import type { Identifier } from "dnd-core";

const REORDER_ITEM_TYPE = "reorder-list-item";

type DragItem = {
  id: string;
  index: number;
  type: typeof REORDER_ITEM_TYPE;
};

export type ReorderableListItem = {
  id: string;
  title: string;
  subtitle?: string | null;
  right?: ReactNode;
};

export default function ReorderableList({
  items,
  activeId,
  onMove,
  onSelect,
  className,
}: {
  items: ReorderableListItem[];
  activeId?: string | null;
  onMove: (fromIndex: number, toIndex: number) => void;
  onSelect?: (id: string) => void;
  className?: string;
}) {
  return (
    <div className={clsx("space-y-2", className)}>
      {items.map((item, index) => (
        <ReorderableRow
          key={item.id}
          item={item}
          index={index}
          active={activeId === item.id}
          onMove={onMove}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}

function ReorderableRow({
  item,
  index,
  active,
  onMove,
  onSelect,
}: {
  item: ReorderableListItem;
  index: number;
  active: boolean;
  onMove: (fromIndex: number, toIndex: number) => void;
  onSelect?: (id: string) => void;
}) {
  const rowRef = useRef<HTMLDivElement | null>(null);
  const handleRef = useRef<HTMLButtonElement | null>(null);

  const [{ handlerId }, drop] = useDrop<DragItem, void, { handlerId: Identifier | null }>({
    accept: REORDER_ITEM_TYPE,
    collect: (monitor) => ({
      handlerId: monitor.getHandlerId(),
    }),
    hover: (dragItem, monitor) => {
      if (!rowRef.current) return;

      const dragIndex = dragItem.index;
      const hoverIndex = index;
      if (dragIndex === hoverIndex) return;

      const hoverBoundingRect = rowRef.current.getBoundingClientRect();
      const hoverMiddleY = (hoverBoundingRect.bottom - hoverBoundingRect.top) / 2;
      const clientOffset = monitor.getClientOffset();
      if (!clientOffset) return;
      const hoverClientY = clientOffset.y - hoverBoundingRect.top;

      if (dragIndex < hoverIndex && hoverClientY < hoverMiddleY) return;
      if (dragIndex > hoverIndex && hoverClientY > hoverMiddleY) return;

      onMove(dragIndex, hoverIndex);
      dragItem.index = hoverIndex;
    },
  });

  const [{ isDragging }, drag, preview] = useDrag({
    type: REORDER_ITEM_TYPE,
    item: (): DragItem => ({ id: item.id, index, type: REORDER_ITEM_TYPE }),
    collect: (monitor) => ({
      isDragging: monitor.isDragging(),
    }),
  });

  const attachRowRef = useCallback(
    (node: HTMLDivElement | null) => {
      rowRef.current = node;
      if (!node) return;
      drop(node);
      preview(node);
    },
    [drop, preview],
  );

  const attachHandleRef = useCallback(
    (node: HTMLButtonElement | null) => {
      handleRef.current = node;
      if (!node) return;
      drag(node);
    },
    [drag],
  );

  return (
    <div
      ref={attachRowRef}
      data-handler-id={handlerId ?? undefined}
      className={clsx(
        "flex items-center justify-between gap-3 rounded-lg border px-3 py-2.5 shadow-xs transition-colors",
        isDragging && "opacity-40",
        active
          ? "border-info-surface-border bg-info-surface"
          : "border-border bg-card hover:bg-muted",
        onSelect && "cursor-pointer",
      )}
      onClick={() => onSelect?.(item.id)}
    >
      <div className="flex min-w-0 items-center gap-3">
        <button
          ref={attachHandleRef}
          type="button"
          className="inline-flex h-8 w-8 items-center justify-center rounded-md border border-border bg-white text-muted-foreground hover:bg-muted"
          aria-label="Drag to reorder"
          title="Drag to reorder"
          onClick={(event) => event.stopPropagation()}
        >
          <svg viewBox="0 0 20 20" fill="currentColor" className="h-4 w-4">
            <path d="M7 4.5A1.5 1.5 0 1 1 4 4.5a1.5 1.5 0 0 1 3 0Zm9 0A1.5 1.5 0 1 1 13 4.5a1.5 1.5 0 0 1 3 0ZM7 10a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Zm9 0a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0ZM7 15.5a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Zm9 0a1.5 1.5 0 1 1-3 0 1.5 1.5 0 0 1 3 0Z" />
          </svg>
        </button>
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold text-card-foreground">{item.title}</div>
          {item.subtitle ? (
            <div className="truncate text-xs text-muted-foreground">{item.subtitle}</div>
          ) : null}
        </div>
      </div>
      {item.right ? <div className="shrink-0">{item.right}</div> : null}
    </div>
  );
}
