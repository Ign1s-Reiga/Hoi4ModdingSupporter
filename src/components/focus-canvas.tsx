'use client';

import * as React from 'react';

import {
  boundsOf,
  canvasSize,
  CELL_HEIGHT,
  CELL_WIDTH,
  layoutFocuses,
  NODE_HEIGHT,
  NODE_WIDTH,
  nodeOrigin,
  type Placement,
} from '@/lib/focus-layout';
import type { Focus } from '@/lib/types';
import { cn } from '@/lib/utils';

/** Reset and first paint both use this. */
const DEFAULT_ZOOM = 1;

interface FocusCanvasProps {
  focuses: Focus[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  /** Called with whole-grid coordinates when a node is dragged. */
  onMove?: (id: string, x: number, y: number) => void;
}

/**
 * A pannable, zoomable view of a focus tree.
 *
 * Nodes sit on the same grid the game uses, so dragging one and saving writes
 * the `x`/`y` a modder would otherwise count out by hand.
 */
export function FocusCanvas({ focuses, selectedId, onSelect, onMove }: FocusCanvasProps) {
  // The canvas grid matches the game at 1:1, so that is where it starts.
  const [zoom, setZoom] = React.useState(DEFAULT_ZOOM);
  const [pan, setPan] = React.useState({ x: 24, y: 24 });
  const [drag, setDrag] = React.useState<{ id: string; x: number; y: number } | null>(null);
  const viewport = React.useRef<HTMLDivElement>(null);

  const placements = React.useMemo(() => layoutFocuses(focuses), [focuses]);
  const bounds = React.useMemo(() => boundsOf(placements.values()), [placements]);
  const size = canvasSize(bounds);

  // While dragging, the node follows the pointer without touching the file.
  const effective = React.useCallback(
    (focus: Focus): Placement => {
      const base = placements.get(focus.id) ?? { x: 0, y: 0 };
      if (drag && drag.id === focus.id) return { x: drag.x, y: drag.y };
      return base;
    },
    [drag, placements],
  );

  function startPan(event: React.PointerEvent) {
    if (event.button !== 0 && event.button !== 1) return;
    // A left drag that starts on a node moves the node instead of the canvas.
    if (event.button === 0 && (event.target as HTMLElement).closest('[data-focus-node]')) {
      return;
    }

    const origin = { x: event.clientX, y: event.clientY };
    const start = { ...pan };
    const element = event.currentTarget as HTMLElement;
    element.setPointerCapture(event.pointerId);

    const move = (moveEvent: PointerEvent) => {
      setPan({
        x: start.x + (moveEvent.clientX - origin.x),
        y: start.y + (moveEvent.clientY - origin.y),
      });
    };
    const end = () => {
      element.removeEventListener('pointermove', move);
      element.removeEventListener('pointerup', end);
    };

    element.addEventListener('pointermove', move);
    element.addEventListener('pointerup', end);
  }

  function startDrag(event: React.PointerEvent, focus: Focus) {
    if (!onMove || event.button !== 0) return;
    event.stopPropagation();

    const placement = placements.get(focus.id) ?? { x: 0, y: 0 };
    const origin = { x: event.clientX, y: event.clientY };
    const element = event.currentTarget as HTMLElement;
    element.setPointerCapture(event.pointerId);

    let latest = placement;

    const move = (moveEvent: PointerEvent) => {
      const deltaX = (moveEvent.clientX - origin.x) / (CELL_WIDTH * zoom);
      const deltaY = (moveEvent.clientY - origin.y) / (CELL_HEIGHT * zoom);
      latest = {
        x: Math.round(placement.x + deltaX),
        y: Math.round(placement.y + deltaY),
      };
      setDrag({ id: focus.id, ...latest });
    };

    const end = () => {
      element.removeEventListener('pointermove', move);
      element.removeEventListener('pointerup', end);
      setDrag(null);

      if (latest.x !== placement.x || latest.y !== placement.y) {
        // The stored value is relative when the focus is anchored to another.
        const anchor = focus.relativePositionId.trim() ? placements.get(focus.relativePositionId.trim()) : undefined;
        const offset = anchor ?? { x: 0, y: 0 };
        onMove(focus.id, latest.x - offset.x, latest.y - offset.y);
      }
    };

    element.addEventListener('pointermove', move);
    element.addEventListener('pointerup', end);
  }

  function onWheel(event: React.WheelEvent) {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    setZoom((current) => Math.min(2, Math.max(0.25, current - event.deltaY * 0.001)));
  }

  const edges = React.useMemo(() => {
    const lines: Array<{ key: string; path: string; kind: 'prerequisite' | 'exclusive' }> = [];

    for (const focus of focuses) {
      const target = effective(focus);
      const targetOrigin = nodeOrigin(target, bounds);

      for (const group of focus.prerequisites) {
        for (const prerequisiteId of group) {
          const source = focuses.find((entry) => entry.id === prerequisiteId);
          if (!source) continue;

          const sourceOrigin = nodeOrigin(effective(source), bounds);
          const startX = sourceOrigin.left + NODE_WIDTH / 2;
          const startY = sourceOrigin.top + NODE_HEIGHT;
          const endX = targetOrigin.left + NODE_WIDTH / 2;
          const endY = targetOrigin.top;
          const midY = startY + (endY - startY) / 2;

          lines.push({
            key: `${prerequisiteId}->${focus.id}`,
            path: `M ${startX} ${startY} V ${midY} H ${endX} V ${endY}`,
            kind: 'prerequisite',
          });
        }
      }

      for (const exclusiveId of focus.mutuallyExclusive) {
        const other = focuses.find((entry) => entry.id === exclusiveId);
        if (!other || other.id < focus.id) continue;

        const otherOrigin = nodeOrigin(effective(other), bounds);
        lines.push({
          key: `${focus.id}<->${exclusiveId}`,
          path: `M ${targetOrigin.left + NODE_WIDTH} ${targetOrigin.top + NODE_HEIGHT / 2} L ${otherOrigin.left} ${otherOrigin.top + NODE_HEIGHT / 2}`,
          kind: 'exclusive',
        });
      }
    }

    return lines;
  }, [bounds, effective, focuses]);

  return (
    <div className='relative h-full overflow-hidden bg-surface-sunken'>
      <div
        ref={viewport}
        className='h-full w-full cursor-grab active:cursor-grabbing'
        onPointerDown={startPan}
        onWheel={onWheel}
      >
        <div
          className='relative origin-top-left'
          style={{
            width: size.width,
            height: size.height,
            transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
          }}
        >
          <svg className='pointer-events-none absolute inset-0' width={size.width} height={size.height}>
            {edges.map((edge) => (
              <path
                key={edge.key}
                d={edge.path}
                fill='none'
                stroke={edge.kind === 'exclusive' ? 'var(--danger)' : 'var(--border-strong)'}
                strokeWidth={edge.kind === 'exclusive' ? 1.5 : 2}
                strokeDasharray={edge.kind === 'exclusive' ? '5 4' : undefined}
              />
            ))}
          </svg>

          {focuses.map((focus) => {
            const origin = nodeOrigin(effective(focus), bounds);
            const isSelected = focus.id === selectedId;

            return (
              <button
                key={focus.id}
                type='button'
                data-focus-node
                onPointerDown={(event) => startDrag(event, focus)}
                onClick={() => onSelect(focus.id)}
                className={cn(
                  'absolute flex flex-col justify-center gap-0.5 rounded-md border px-2 text-left transition-colors',
                  isSelected
                    ? 'border-accent bg-accent-soft shadow-[0_0_0_1px_var(--accent)]'
                    : 'border-border bg-surface hover:border-border-strong',
                )}
                style={{
                  left: origin.left,
                  top: origin.top,
                  width: NODE_WIDTH,
                  height: NODE_HEIGHT,
                }}
                title={focus.id}
              >
                <span className='truncate font-mono text-[0.6875rem] leading-tight text-foreground'>
                  {focus.id || '(no id)'}
                </span>
                <span className='truncate text-[0.625rem] text-muted'>
                  {focus.cost ? `${focus.cost} days` : 'no cost'}
                  {focus.shared ? ' · shared' : ''}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <div className='pointer-events-none absolute bottom-2 left-2 rounded-md bg-surface/85 px-2 py-1 text-[0.6875rem] text-muted'>
        Drag a focus to move it · Ctrl + wheel to zoom · drag the background to pan
      </div>
      <div className='absolute right-2 top-2 flex items-center gap-1 rounded-md border border-border bg-surface/85 px-1.5 py-1 text-[0.6875rem] text-muted'>
        {Math.round(zoom * 100)}%
        <button
          type='button'
          className='ml-1 rounded px-1 hover:bg-surface-raised hover:text-foreground'
          onClick={() => {
            setZoom(DEFAULT_ZOOM);
            setPan({ x: 24, y: 24 });
          }}
        >
          reset
        </button>
      </div>
    </div>
  );
}
