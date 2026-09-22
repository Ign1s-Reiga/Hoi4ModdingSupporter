'use client';

import * as React from 'react';
import { ImageOff } from 'lucide-react';

import { boundsOf, canvasSize, CELL_HEIGHT, CELL_WIDTH, NODE_HEIGHT, NODE_WIDTH, nodeOrigin } from '@/lib/focus-layout';
import type { SpriteIcon } from '@/lib/types';
import { cn } from '@/lib/utils';

/** Reset and first paint both use this. */
const DEFAULT_ZOOM = 1;

/** One box on the grid, at whole-grid coordinates. */
export interface TreeNode {
  id: string;
  x: number;
  y: number;
  label: string;
  sublabel?: string;
  /** Art drawn above the label, or undefined for a placeholder. */
  icon?: SpriteIcon;
  title?: string;
}

/** A line between two nodes: a requirement drawn top-down, or an exclusion. */
export interface TreeEdge {
  key: string;
  from: string;
  to: string;
  kind: 'prerequisite' | 'exclusive';
}

interface TreeCanvasProps {
  nodes: TreeNode[];
  edges: TreeEdge[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  /** Called with whole-grid coordinates when a node is dragged. */
  onMove?: (id: string, x: number, y: number) => void;
  /** What the corner hint calls a node: "focus", "technology". */
  noun?: string;
}

/**
 * A pannable, zoomable view of a tree of nodes on the game's grid.
 *
 * Nodes sit on the grid the game uses, so dragging one and saving writes the
 * `x`/`y` a modder would otherwise count out by hand. Each carries the art
 * the game would draw, which is how a tree is read at a glance: the ids alone
 * all look alike from a step back. A focus tree and a technology tree are
 * both this, with their own way of turning a node's fields into a place.
 */
export function TreeCanvas({ nodes, edges, selectedId, onSelect, onMove, noun = 'node' }: TreeCanvasProps) {
  // The canvas grid matches the game at 1:1, so that is where it starts.
  const [zoom, setZoom] = React.useState(DEFAULT_ZOOM);
  const [pan, setPan] = React.useState({ x: 24, y: 24 });
  const [drag, setDrag] = React.useState<{ id: string; x: number; y: number } | null>(null);
  const viewport = React.useRef<HTMLDivElement>(null);

  const byId = React.useMemo(() => new Map(nodes.map((node) => [node.id, node])), [nodes]);
  const bounds = React.useMemo(() => boundsOf(nodes), [nodes]);
  const size = canvasSize(bounds);

  // While dragging, the node follows the pointer without touching the file.
  const effective = React.useCallback(
    (node: TreeNode) => {
      if (drag && drag.id === node.id) return { x: drag.x, y: drag.y };
      return { x: node.x, y: node.y };
    },
    [drag],
  );

  function startPan(event: React.PointerEvent) {
    if (event.button !== 0 && event.button !== 1) return;
    // A left drag that starts on a node moves the node instead of the canvas.
    if (event.button === 0 && (event.target as HTMLElement).closest('[data-tree-node]')) {
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

  function startDrag(event: React.PointerEvent, node: TreeNode) {
    if (!onMove || event.button !== 0) return;
    event.stopPropagation();

    const placement = { x: node.x, y: node.y };
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
      setDrag({ id: node.id, ...latest });
    };

    const end = () => {
      element.removeEventListener('pointermove', move);
      element.removeEventListener('pointerup', end);
      setDrag(null);

      if (latest.x !== placement.x || latest.y !== placement.y) {
        onMove(node.id, latest.x, latest.y);
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

  const lines = React.useMemo(() => {
    const drawn: Array<{ key: string; path: string; kind: TreeEdge['kind'] }> = [];

    for (const edge of edges) {
      const from = byId.get(edge.from);
      const to = byId.get(edge.to);
      if (!from || !to) continue;

      const fromOrigin = nodeOrigin(effective(from), bounds);
      const toOrigin = nodeOrigin(effective(to), bounds);

      if (edge.kind === 'prerequisite') {
        const startX = fromOrigin.left + NODE_WIDTH / 2;
        const startY = fromOrigin.top + NODE_HEIGHT;
        const endX = toOrigin.left + NODE_WIDTH / 2;
        const endY = toOrigin.top;
        const midY = startY + (endY - startY) / 2;
        drawn.push({ key: edge.key, path: `M ${startX} ${startY} V ${midY} H ${endX} V ${endY}`, kind: edge.kind });
      } else {
        drawn.push({
          key: edge.key,
          path: `M ${fromOrigin.left + NODE_WIDTH} ${fromOrigin.top + NODE_HEIGHT / 2} L ${toOrigin.left} ${toOrigin.top + NODE_HEIGHT / 2}`,
          kind: edge.kind,
        });
      }
    }

    return drawn;
  }, [bounds, byId, edges, effective]);

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
            {lines.map((line) => (
              <path
                key={line.key}
                d={line.path}
                fill='none'
                stroke={line.kind === 'exclusive' ? 'var(--danger)' : 'var(--border-strong)'}
                strokeWidth={line.kind === 'exclusive' ? 1.5 : 2}
                strokeDasharray={line.kind === 'exclusive' ? '5 4' : undefined}
              />
            ))}
          </svg>

          {nodes.map((node) => {
            const origin = nodeOrigin(effective(node), bounds);
            const isSelected = node.id === selectedId;

            return (
              <button
                key={node.id}
                type='button'
                data-tree-node
                data-focus-node
                onPointerDown={(event) => startDrag(event, node)}
                onClick={() => onSelect(node.id)}
                className={cn(
                  'absolute flex flex-col items-center justify-center gap-1 rounded-md border px-2 py-1.5 transition-colors',
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
                title={node.title ?? node.id}
              >
                <span className='flex h-9 items-center justify-center'>
                  {node.icon?.url ? (
                    // Data URL produced by the backend, so next/image cannot help here.
                    // oxlint-disable-next-line next/no-img-element
                    <img
                      src={node.icon.url}
                      alt=''
                      // Without this the browser drags the picture out of the
                      // node instead of moving it.
                      draggable={false}
                      className='max-h-9 max-w-full object-contain'
                    />
                  ) : (
                    <ImageOff className='size-4 text-border-strong' />
                  )}
                </span>
                <span className='w-full truncate text-center font-mono text-[0.6875rem] leading-tight text-foreground'>
                  {node.label || '(no id)'}
                </span>
                <span className='w-full truncate text-center text-[0.625rem] text-muted'>{node.sublabel ?? ''}</span>
              </button>
            );
          })}
        </div>
      </div>

      <div className='pointer-events-none absolute bottom-2 left-2 rounded-md bg-surface/85 px-2 py-1 text-[0.6875rem] text-muted'>
        Drag a {noun} to move it · Ctrl + wheel to zoom · drag the background to pan
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
