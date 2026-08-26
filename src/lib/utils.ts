import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatTimestamp(milliseconds: number): string {
  if (!milliseconds) return 'unknown';

  const date = new Date(milliseconds);
  const elapsed = Date.now() - milliseconds;
  const day = 24 * 60 * 60 * 1000;

  if (elapsed < day) {
    return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  }
  if (elapsed < 7 * day) {
    return date.toLocaleDateString(undefined, { weekday: 'short', hour: '2-digit', minute: '2-digit' });
  }
  return date.toLocaleDateString();
}

/** Last segment of a path that uses either separator. */
export function basename(path: string): string {
  const parts = path.replace(/\\/g, '/').split('/');
  return parts[parts.length - 1] ?? path;
}

export function dirname(path: string): string {
  const normalised = path.replace(/\\/g, '/');
  const index = normalised.lastIndexOf('/');
  return index <= 0 ? normalised : normalised.slice(0, index);
}

/** Groups project files by their first path segment. */
export function topLevelFolder(relativePath: string): string {
  const index = relativePath.indexOf('/');
  return index === -1 ? '' : relativePath.slice(0, index);
}

export function isInFolder(relativePath: string, folder: string): boolean {
  if (!folder) return true;
  const lower = relativePath.toLowerCase();
  const prefix = folder.toLowerCase();
  return lower === prefix || lower.startsWith(`${prefix}/`);
}
