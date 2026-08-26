'use client';

import * as React from 'react';
import * as LabelPrimitive from '@radix-ui/react-label';

import { cn } from '@/lib/utils';

const fieldSurface =
  'w-full rounded-md border border-border bg-surface px-2.5 text-sm text-foreground placeholder:text-muted/70 transition-colors hover:border-border-strong focus:border-accent focus:outline-none disabled:cursor-not-allowed disabled:opacity-50';

export function Input({ className, ...props }: React.ComponentProps<'input'>) {
  return <input className={cn(fieldSurface, 'h-9', className)} {...props} />;
}

/** Monospace variant for ids, coordinates and other script values. */
export function CodeInput({ className, ...props }: React.ComponentProps<'input'>) {
  return (
    <input
      spellCheck={false}
      autoComplete='off'
      className={cn(fieldSurface, 'h-9 font-mono text-[0.8125rem]', className)}
      {...props}
    />
  );
}

export function Textarea({ className, ...props }: React.ComponentProps<'textarea'>) {
  return (
    <textarea
      spellCheck={false}
      className={cn(fieldSurface, 'min-h-24 resize-y py-2 font-mono text-[0.8125rem] leading-relaxed', className)}
      {...props}
    />
  );
}

export function Label({ className, ...props }: React.ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      className={cn('text-xs font-medium tracking-wide text-muted uppercase', className)}
      {...props}
    />
  );
}

export function Field({
  label,
  hint,
  className,
  children,
}: {
  label: string;
  hint?: string;
  className?: string;
  children: React.ReactNode;
}) {
  const id = React.useId();

  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <Label htmlFor={id}>{label}</Label>
      {React.isValidElement<{ id?: string }>(children) ? React.cloneElement(children, { id }) : children}
      {hint ? <p className='text-xs text-muted'>{hint}</p> : null}
    </div>
  );
}
