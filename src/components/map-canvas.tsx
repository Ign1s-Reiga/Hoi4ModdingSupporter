'use client';

import * as React from 'react';
import { Loader2, Minus, Plus } from 'lucide-react';

import { Button } from '@/components/ui/button';

export interface MapPickPoint {
  x: number;
  y: number;
}

/**
 * A pannable, zoomable view of the province map.
 *
 * The image is the full sized render from the backend, so a click is converted
 * straight back into map pixels and resolved there — the frontend never needs
 * the thirteen million pixel province index.
 */
export function MapCanvas({
  source,
  width,
  height,
  marker,
  onPick,
  isBusy,
}: {
  source: string | null;
  width: number;
  height: number;
  marker: MapPickPoint | null;
  onPick: (point: MapPickPoint) => void;
  isBusy?: boolean;
}) {
  const viewport = React.useRef<HTMLDivElement>(null);
  const [zoom, setZoom] = React.useState(0);
  const [pan, setPan] = React.useState({ x: 0, y: 0 });
  const dragged = React.useRef(false);

  // Zoom starts at zero meaning "not measured yet"; the first paint fits the
  // whole map to the pane so a 5632 pixel wide bitmap is not shown 1:1.
  React.useEffect(() => {
    const element = viewport.current;
    if (!element || !width) return;

    const fit = Math.min(element.clientWidth / width, element.clientHeight / height);
    setZoom(fit > 0 ? fit : 0.1);
    setPan({
      x: Math.max(0, (element.clientWidth - width * fit) / 2),
      y: Math.max(0, (element.clientHeight - height * fit) / 2),
    });
  }, [height, width]);

  function startPan(event: React.PointerEvent) {
    if (event.button !== 0 && event.button !== 1) return;

    dragged.current = false;
    const origin = { x: event.clientX, y: event.clientY };
    const start = { ...pan };
    const element = event.currentTarget as HTMLElement;
    element.setPointerCapture(event.pointerId);

    const move = (moveEvent: PointerEvent) => {
      const dx = moveEvent.clientX - origin.x;
      const dy = moveEvent.clientY - origin.y;
      // A few pixels of slip should still count as a click, not a drag.
      if (Math.abs(dx) > 3 || Math.abs(dy) > 3) dragged.current = true;
      setPan({ x: start.x + dx, y: start.y + dy });
    };
    const end = () => {
      element.removeEventListener('pointermove', move);
      element.removeEventListener('pointerup', end);
    };

    element.addEventListener('pointermove', move);
    element.addEventListener('pointerup', end);
  }

  function pickAt(event: React.MouseEvent) {
    if (dragged.current || !zoom) return;

    const element = viewport.current;
    if (!element) return;

    const bounds = element.getBoundingClientRect();
    const x = Math.floor((event.clientX - bounds.left - pan.x) / zoom);
    const y = Math.floor((event.clientY - bounds.top - pan.y) / zoom);

    if (x < 0 || y < 0 || x >= width || y >= height) return;
    onPick({ x, y });
  }

  /** Keeps the point under the cursor fixed while the scale changes. */
  function zoomBy(factor: number, anchor?: { x: number; y: number }) {
    const element = viewport.current;
    if (!element || !zoom) return;

    const bounds = element.getBoundingClientRect();
    const point = anchor ?? {
      x: bounds.left + bounds.width / 2,
      y: bounds.top + bounds.height / 2,
    };
    const next = Math.min(8, Math.max(0.05, zoom * factor));

    const mapX = (point.x - bounds.left - pan.x) / zoom;
    const mapY = (point.y - bounds.top - pan.y) / zoom;

    setZoom(next);
    setPan({
      x: point.x - bounds.left - mapX * next,
      y: point.y - bounds.top - mapY * next,
    });
  }

  function onWheel(event: React.WheelEvent) {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault();
    zoomBy(event.deltaY < 0 ? 1.15 : 1 / 1.15, { x: event.clientX, y: event.clientY });
  }

  return (
    <div className='relative h-full overflow-hidden bg-surface-sunken'>
      <div
        ref={viewport}
        className='h-full w-full cursor-grab active:cursor-grabbing'
        onPointerDown={startPan}
        onClick={pickAt}
        onWheel={onWheel}
      >
        {source ? (
          <div
            className='relative origin-top-left'
            style={{
              width,
              height,
              transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
            }}
          >
            {/* The render is a data URL from the backend, not a served asset. */}
            {/* oxlint-disable-next-line next/no-img-element */}
            <img
              src={source}
              alt=''
              width={width}
              height={height}
              draggable={false}
              className='max-w-none select-none'
              style={{ imageRendering: 'pixelated' }}
            />

            {marker ? (
              <span
                className='pointer-events-none absolute rounded-full border-2 border-accent bg-accent/30'
                style={{
                  left: marker.x - 6 / zoom,
                  top: marker.y - 6 / zoom,
                  width: 12 / zoom,
                  height: 12 / zoom,
                  borderWidth: Math.max(1, 2 / zoom),
                }}
              />
            ) : null}
          </div>
        ) : null}
      </div>

      {isBusy ? (
        <div className='absolute inset-0 flex items-center justify-center gap-2 bg-surface-sunken/80 text-sm text-muted'>
          <Loader2 className='size-4 animate-spin' />
          Painting the map…
        </div>
      ) : null}

      <div className='absolute right-2 top-2 flex items-center gap-1 rounded-md border border-border bg-surface/85 px-1.5 py-1 text-[0.6875rem] text-muted'>
        <Button variant='ghost' size='icon-sm' title='Zoom out' onClick={() => zoomBy(1 / 1.3)}>
          <Minus />
        </Button>
        <span className='w-10 text-center'>{Math.round(zoom * 100)}%</span>
        <Button variant='ghost' size='icon-sm' title='Zoom in' onClick={() => zoomBy(1.3)}>
          <Plus />
        </Button>
      </div>

      <div className='pointer-events-none absolute bottom-2 left-2 rounded-md bg-surface/85 px-2 py-1 text-[0.6875rem] text-muted'>
        Click a province to select it · drag to pan · Ctrl + wheel to zoom
      </div>
    </div>
  );
}
