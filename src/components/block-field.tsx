'use client';

import { Textarea } from '@/components/ui/form';

/** A script block shown as plain text, collapsed unless it has content. */
export function BlockField({
  label,
  value,
  onChange,
  open,
  placeholder = 'add_political_power = 120',
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  open?: boolean;
  placeholder?: string;
}) {
  return (
    <details open={open || value.trim().length > 0} className='group'>
      <summary className='cursor-pointer list-none text-xs font-medium uppercase tracking-wide text-muted marker:content-none hover:text-foreground'>
        {label}
        {value.trim() ? '' : ' · empty'}
      </summary>
      <Textarea
        className='mt-1.5'
        rows={4}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
      />
    </details>
  );
}
