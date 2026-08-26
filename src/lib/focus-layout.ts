import type { Focus } from './types';

/** Grid spacing of the canvas, roughly matching how the game lays a tree out. */
export const CELL_WIDTH = 124;
export const CELL_HEIGHT = 148;
export const NODE_WIDTH = 104;
export const NODE_HEIGHT = 64;

export interface Placement {
  x: number;
  y: number;
}

function toNumber(value: string): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : 0;
}

/**
 * Resolves every focus to absolute grid coordinates.
 *
 * A focus with `relative_position_id` is positioned against another focus, and
 * those chains can nest, so each one is resolved once and memoised. A cycle or
 * a missing target falls back to the raw coordinates rather than throwing.
 */
export function layoutFocuses(focuses: Focus[]): Map<string, Placement> {
  const byId = new Map(focuses.map((focus) => [focus.id, focus]));
  const resolved = new Map<string, Placement>();

  const resolve = (focus: Focus, seen: Set<string>): Placement => {
    const cached = resolved.get(focus.id);
    if (cached) return cached;

    const own = { x: toNumber(focus.x), y: toNumber(focus.y) };
    const anchorId = focus.relativePositionId.trim();
    let placement = own;

    if (anchorId && anchorId !== focus.id && !seen.has(focus.id)) {
      const anchor = byId.get(anchorId);
      if (anchor) {
        seen.add(focus.id);
        const base = resolve(anchor, seen);
        placement = { x: base.x + own.x, y: base.y + own.y };
      }
    }

    resolved.set(focus.id, placement);
    return placement;
  };

  for (const focus of focuses) {
    resolve(focus, new Set());
  }

  return resolved;
}

export interface Bounds {
  minX: number;
  minY: number;
  maxX: number;
  maxY: number;
}

export function boundsOf(placements: Iterable<Placement>): Bounds {
  let minX = Number.POSITIVE_INFINITY;
  let minY = Number.POSITIVE_INFINITY;
  let maxX = Number.NEGATIVE_INFINITY;
  let maxY = Number.NEGATIVE_INFINITY;

  for (const placement of placements) {
    minX = Math.min(minX, placement.x);
    minY = Math.min(minY, placement.y);
    maxX = Math.max(maxX, placement.x);
    maxY = Math.max(maxY, placement.y);
  }

  if (!Number.isFinite(minX)) {
    return { minX: 0, minY: 0, maxX: 0, maxY: 0 };
  }

  return { minX, minY, maxX, maxY };
}

/** Top-left pixel of a node, given the grid origin. */
export function nodeOrigin(placement: Placement, bounds: Bounds) {
  return {
    left: (placement.x - bounds.minX) * CELL_WIDTH,
    top: (placement.y - bounds.minY) * CELL_HEIGHT,
  };
}

export function canvasSize(bounds: Bounds) {
  return {
    width: (bounds.maxX - bounds.minX + 1) * CELL_WIDTH + NODE_WIDTH,
    height: (bounds.maxY - bounds.minY + 1) * CELL_HEIGHT + NODE_HEIGHT,
  };
}
