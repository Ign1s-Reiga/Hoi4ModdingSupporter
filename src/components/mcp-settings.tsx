'use client';

import * as React from 'react';
import { Copy, Eye, EyeOff, Loader2, RefreshCw } from 'lucide-react';
import { toast } from 'sonner';

import { Button } from '@/components/ui/button';
import { Input, Label } from '@/components/ui/form';
import { Badge, PanelBody } from '@/components/ui/panel';
import { confirmDisconnect } from '@/lib/dialogs';
import { api, describeError } from '@/lib/ipc';
import { useAppStore } from '@/lib/store';
import type { McpSettings, McpStatus } from '@/lib/types';

/** The name the client will know this server by. */
const CLIENT_NAME = 'hoi4-modding-supporter';

/**
 * Settings for the MCP server: on or off, which port, and the token a client
 * needs. The `claude mcp add` line is spelled out because that is what the
 * user will paste; nobody should have to assemble it from three fields.
 */
export function McpSettingsSection({ saved }: { saved: McpSettings }) {
  const { setMcp, reloadSettings, setError } = useAppStore();

  const [status, setStatus] = React.useState<McpStatus | null>(null);
  const [port, setPort] = React.useState(String(saved.port));
  const [revealed, setRevealed] = React.useState(false);
  const [busy, setBusy] = React.useState(false);

  const refresh = React.useCallback(async () => {
    try {
      setStatus(await api.mcpStatus());
    } catch (error) {
      setError(describeError(error));
    }
  }, [setError]);

  // Re-asked whenever the saved settings change: the backend restarts the
  // server on a save, and the status should say what came of it.
  React.useEffect(() => {
    let cancelled = false;
    api
      .mcpStatus()
      .then((status) => {
        if (!cancelled) setStatus(status);
      })
      .catch((error: unknown) => {
        if (!cancelled) setError(describeError(error));
      });

    return () => {
      cancelled = true;
    };
  }, [saved, setError]);

  async function save(changes: Partial<McpSettings>) {
    setBusy(true);
    try {
      await setMcp({ ...saved, ...changes });
      await refresh();
    } catch (error) {
      setError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function regenerate() {
    const sure = await confirmDisconnect(
      'Replace the token? Every client configured with the current one stops working until it is updated.',
    );
    if (!sure) return;

    setBusy(true);
    try {
      setStatus(await api.mcpRegenerateToken());
      // The backend changed the token behind the store's back.
      await reloadSettings();
      toast.success('New token issued');
    } catch (error) {
      setError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`${what} copied`);
    } catch {
      toast.error('The clipboard is not available');
    }
  }

  const token = status?.token ?? saved.token;
  const address = status?.address ?? `http://127.0.0.1:${saved.port}/mcp`;
  const command = `claude mcp add --transport http ${CLIENT_NAME} ${address} --header "Authorization: Bearer ${token}"`;
  const masked = token.replace(/./g, '•');
  const portNumber = Number.parseInt(port, 10);
  const portValid = Number.isInteger(portNumber) && portNumber >= 1024 && portNumber <= 65535;

  return (
    <PanelBody className='flex flex-col gap-4 p-4'>
      <div className='flex items-center gap-2'>
        <Button
          variant={saved.enabled ? 'primary' : 'default'}
          onClick={() => void save({ enabled: true })}
          disabled={busy || saved.enabled}
        >
          Enabled
        </Button>
        <Button
          variant={saved.enabled ? 'default' : 'primary'}
          onClick={() => void save({ enabled: false })}
          disabled={busy || !saved.enabled}
        >
          Disabled
        </Button>

        <span className='ml-auto flex items-center gap-2 text-xs text-muted'>
          {status === null ? (
            <Loader2 className='size-3.5 animate-spin' />
          ) : status.running ? (
            <>
              <Badge tone='accent'>Running</Badge>
              <span data-selectable className='font-mono'>
                {status.address}
              </span>
              <span>· {status.toolCount} tools</span>
            </>
          ) : status.enabled ? (
            <Badge tone='danger'>Not running — see the console</Badge>
          ) : (
            <Badge>Off</Badge>
          )}
        </span>
      </div>

      <div className='flex flex-col gap-1.5'>
        <Label htmlFor='mcp-port'>Port</Label>
        <div className='flex gap-2'>
          <Input
            id='mcp-port'
            inputMode='numeric'
            value={port}
            onChange={(event) => setPort(event.target.value)}
            className='w-32 font-mono text-xs'
          />
          <Button
            variant='primary'
            onClick={() => void save({ port: portNumber })}
            disabled={busy || !portValid || portNumber === saved.port}
          >
            {busy ? <Loader2 className='animate-spin' /> : null}
            Save
          </Button>
        </div>
        <p className='text-xs text-muted'>
          {portValid
            ? 'Loopback only: nothing outside this machine can reach it.'
            : 'Pick a port between 1024 and 65535.'}
        </p>
      </div>

      <div className='flex flex-col gap-1.5'>
        <Label>Token</Label>
        <div className='flex items-center gap-2'>
          <code
            data-selectable={revealed ? true : undefined}
            className='min-w-0 flex-1 truncate rounded-md border border-border bg-surface-sunken px-2.5 py-2 font-mono text-xs'
          >
            {revealed ? token : masked}
          </code>
          <Button
            size='icon'
            title={revealed ? 'Hide the token' : 'Show the token'}
            onClick={() => setRevealed((value) => !value)}
          >
            {revealed ? <EyeOff /> : <Eye />}
          </Button>
          <Button size='icon' title='Copy the token' onClick={() => void copy(token, 'Token')}>
            <Copy />
          </Button>
          <Button title='Issue a new token' onClick={() => void regenerate()} disabled={busy}>
            <RefreshCw />
            Regenerate
          </Button>
        </div>
        <p className='text-xs text-muted'>
          Every request must carry it. Regenerating cuts off any client still using the old one.
        </p>
      </div>

      <div className='flex flex-col gap-1.5'>
        <div className='flex items-center justify-between'>
          <Label>Connect Claude Code</Label>
          <Button size='sm' onClick={() => void copy(command, 'Command')}>
            <Copy />
            Copy command
          </Button>
        </div>
        <pre className='overflow-x-auto rounded-md border border-border bg-surface-sunken px-3 py-2 font-mono text-[0.6875rem] leading-relaxed'>
          {revealed ? command : command.replace(token, masked)}
        </pre>
        <p className='text-xs text-muted'>
          Run it once in a terminal; Claude Code then finds this app whenever the window is open. Other MCP clients take
          the same URL and header.
        </p>
      </div>
    </PanelBody>
  );
}
