/**
 * Renders the application icon to `assets/app-icon.png`.
 *
 * The icon is drawn procedurally with signed distance fields and encoded as a
 * PNG through Node's own zlib, so the repository needs no image tooling and the
 * source of the artwork stays reviewable as text.
 *
 *   node scripts/generate-icon.mjs
 *   pnpm tauri icon assets/app-icon.png
 */

import { deflateSync, crc32 as zlibCrc32 } from 'node:zlib';
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SIZE = 1024;
const OUTPUT = resolve(dirname(fileURLToPath(import.meta.url)), '..', 'assets', 'app-icon.png');

const BACKGROUND_TOP = [32, 42, 61];
const BACKGROUND_BOTTOM = [17, 22, 33];
const GOLD = [232, 180, 74];
const GOLD_DIM = [176, 132, 52];

/** Rounded rectangle centred on the canvas. */
function roundedRect(x, y, centreX, centreY, halfWidth, halfHeight, radius) {
  const dx = Math.abs(x - centreX) - (halfWidth - radius);
  const dy = Math.abs(y - centreY) - (halfHeight - radius);
  const outside = Math.hypot(Math.max(dx, 0), Math.max(dy, 0));
  return outside + Math.min(Math.max(dx, dy), 0) - radius;
}

function circle(x, y, centreX, centreY, radius) {
  return Math.hypot(x - centreX, y - centreY) - radius;
}

/** Capsule between two points, used for the connecting lines. */
function segment(x, y, ax, ay, bx, by, radius) {
  const px = x - ax;
  const py = y - ay;
  const dx = bx - ax;
  const dy = by - ay;
  const length = dx * dx + dy * dy;
  const t = length === 0 ? 0 : Math.min(1, Math.max(0, (px * dx + py * dy) / length));
  return Math.hypot(px - dx * t, py - dy * t) - radius;
}

/** Distance of a ring outline of the given stroke width. */
function ring(distance, width) {
  return Math.abs(distance) - width / 2;
}

/** Antialiased coverage for a signed distance, 1 inside and 0 outside. */
function coverage(distance) {
  return Math.min(1, Math.max(0, 0.5 - distance));
}

function blend(target, offset, colour, alpha) {
  if (alpha <= 0) return;
  for (let channel = 0; channel < 3; channel += 1) {
    const existing = target[offset + channel];
    target[offset + channel] = Math.round(existing * (1 - alpha) + colour[channel] * alpha);
  }
  target[offset + 3] = Math.round(target[offset + 3] * (1 - alpha) + 255 * alpha);
}

function render() {
  const pixels = new Uint8Array(SIZE * SIZE * 4);
  const centre = SIZE / 2;

  // Focus tree layout: one unlocked focus branching into two follow-ups.
  const root = { x: centre, y: 322, radius: 92 };
  const left = { x: 318, y: 706, radius: 78 };
  const right = { x: 706, y: 706, radius: 78 };
  const junction = 520;

  for (let y = 0; y < SIZE; y += 1) {
    for (let x = 0; x < SIZE; x += 1) {
      const offset = (y * SIZE + x) * 4;
      const px = x + 0.5;
      const py = y + 0.5;

      const plate = roundedRect(px, py, centre, centre, centre - 24, centre - 24, 190);
      const plateCoverage = coverage(plate);
      if (plateCoverage > 0) {
        const mix = y / SIZE;
        const background = BACKGROUND_TOP.map((channel, index) =>
          Math.round(channel * (1 - mix) + BACKGROUND_BOTTOM[index] * mix),
        );
        blend(pixels, offset, background, plateCoverage);
        blend(pixels, offset, GOLD_DIM, coverage(ring(plate, 8)) * 0.55);
      }

      // Connectors are drawn first so the nodes sit on top of them.
      const connectors = Math.min(
        segment(px, py, root.x, root.y + root.radius, root.x, junction, 15),
        segment(px, py, left.x, junction, right.x, junction, 15),
        Math.min(
          segment(px, py, left.x, junction, left.x, left.y - left.radius, 15),
          segment(px, py, right.x, junction, right.x, right.y - right.radius, 15),
        ),
      );
      blend(pixels, offset, GOLD_DIM, coverage(connectors));

      // The completed focus is solid, the two it unlocks are outlines.
      blend(pixels, offset, GOLD, coverage(circle(px, py, root.x, root.y, root.radius)));
      for (const node of [left, right]) {
        const outline = ring(circle(px, py, node.x, node.y, node.radius), 26);
        blend(pixels, offset, GOLD, coverage(outline));
      }
    }
  }

  return pixels;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);

  const payload = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const checksum = Buffer.alloc(4);
  checksum.writeUInt32BE(zlibCrc32(payload) >>> 0, 0);

  return Buffer.concat([length, payload, checksum]);
}

function encodePng(pixels) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(SIZE, 0);
  header.writeUInt32BE(SIZE, 4);
  header[8] = 8; // bit depth
  header[9] = 6; // colour type: RGBA
  header[10] = 0; // deflate
  header[11] = 0; // adaptive filtering
  header[12] = 0; // no interlacing

  // Each scanline is prefixed with its filter type, here always "none".
  const stride = SIZE * 4;
  const raw = Buffer.alloc((stride + 1) * SIZE);
  for (let y = 0; y < SIZE; y += 1) {
    raw[y * (stride + 1)] = 0;
    Buffer.from(pixels.buffer, y * stride, stride).copy(raw, y * (stride + 1) + 1);
  }

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

mkdirSync(dirname(OUTPUT), { recursive: true });
writeFileSync(OUTPUT, encodePng(render()));
console.log(`wrote ${OUTPUT}`);
