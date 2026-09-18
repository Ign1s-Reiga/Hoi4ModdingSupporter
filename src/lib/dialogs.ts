import { confirm } from '@tauri-apps/plugin-dialog';

/**
 * `window.confirm` is not dependable inside a WebView, so confirmations go
 * through the Tauri dialog plugin and show a real native prompt.
 */
export async function confirmDiscard(message: string): Promise<boolean> {
  return confirm(message, { title: 'Unsaved changes', kind: 'warning' });
}

export async function confirmDelete(message: string): Promise<boolean> {
  return confirm(message, { title: 'Delete', kind: 'warning' });
}

/** For actions that disconnect something, such as replacing the MCP token. */
export async function confirmDisconnect(message: string): Promise<boolean> {
  return confirm(message, { title: 'Disconnect clients', kind: 'warning' });
}
