'use client';

import * as React from 'react';

import { TreeCanvas, type TreeEdge, type TreeNode } from '@/components/tree-canvas';
import { layoutFocuses } from '@/lib/focus-layout';
import type { Focus } from '@/lib/types';
import { spriteName, type SpriteMap } from '@/lib/use-sprite-icons';

interface FocusCanvasProps {
  focuses: Focus[];
  /** Art for the `icon` of each focus, keyed by sprite name. */
  icons: SpriteMap;
  selectedId: string | null;
  onSelect: (id: string) => void;
  /** Called with the focus's own `x`/`y` when a node is dragged. */
  onMove?: (id: string, x: number, y: number) => void;
}

/**
 * A focus tree on the shared tree canvas.
 *
 * A focus with `relative_position_id` sits against another focus, so its
 * place on the grid is worked out here and a drag is handed back as the
 * offset the file stores, not the absolute cell it landed on.
 */
export function FocusCanvas({ focuses, icons, selectedId, onSelect, onMove }: FocusCanvasProps) {
  const placements = React.useMemo(() => layoutFocuses(focuses), [focuses]);

  const nodes = React.useMemo<TreeNode[]>(
    () =>
      focuses.map((focus) => {
        const placement = placements.get(focus.id) ?? { x: 0, y: 0 };
        return {
          id: focus.id,
          x: placement.x,
          y: placement.y,
          label: focus.id,
          sublabel: `${focus.cost ? `${focus.cost} days` : 'no cost'}${focus.shared ? ' · shared' : ''}`,
          icon: icons.get(spriteName(focus.icon)),
          title: focus.icon ? `${focus.id} · ${focus.icon}` : focus.id,
        };
      }),
    [focuses, icons, placements],
  );

  const edges = React.useMemo<TreeEdge[]>(() => {
    const lines: TreeEdge[] = [];
    for (const focus of focuses) {
      for (const group of focus.prerequisites) {
        for (const prerequisiteId of group) {
          lines.push({
            key: `${prerequisiteId}->${focus.id}`,
            from: prerequisiteId,
            to: focus.id,
            kind: 'prerequisite',
          });
        }
      }
      for (const exclusiveId of focus.mutuallyExclusive) {
        // Each pair is drawn once, from the id that sorts first.
        if (exclusiveId < focus.id) continue;
        lines.push({ key: `${focus.id}<->${exclusiveId}`, from: focus.id, to: exclusiveId, kind: 'exclusive' });
      }
    }
    return lines;
  }, [focuses]);

  const move = React.useCallback(
    (id: string, x: number, y: number) => {
      if (!onMove) return;
      const focus = focuses.find((entry) => entry.id === id);
      // The stored value is relative when the focus is anchored to another.
      const anchorId = focus?.relativePositionId.trim();
      const anchor = anchorId ? placements.get(anchorId) : undefined;
      const offset = anchor ?? { x: 0, y: 0 };
      onMove(id, x - offset.x, y - offset.y);
    },
    [focuses, onMove, placements],
  );

  return (
    <TreeCanvas
      nodes={nodes}
      edges={edges}
      selectedId={selectedId}
      onSelect={onSelect}
      onMove={onMove ? move : undefined}
      noun='focus'
    />
  );
}
