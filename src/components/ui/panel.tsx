'use client';

import * as React from 'react';
import * as SeparatorPrimitive from '@radix-ui/react-separator';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

/** A bordered surface. Panels are the building block of every workspace pane. */
export function Panel({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn('flex min-h-0 flex-col overflow-hidden rounded-lg border border-border bg-surface', className)}
      {...props}
    />
  );
}

export function PanelHeader({
  title,
  subtitle,
  actions,
  className,
}: {
  title: React.ReactNode;
  subtitle?: React.ReactNode;
  actions?: React.ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn('flex h-11 shrink-0 items-center gap-3 border-b border-border bg-surface-raised px-3', className)}
    >
      <div className='flex min-w-0 flex-col'>
        <span className='truncate text-sm font-medium'>{title}</span>
        {subtitle ? <span className='truncate text-xs text-muted'>{subtitle}</span> : null}
      </div>
      {actions ? <div className='ml-auto flex items-center gap-1.5'>{actions}</div> : null}
    </div>
  );
}

export function PanelBody({ className, ...props }: React.ComponentProps<'div'>) {
  return <div className={cn('min-h-0 flex-1 overflow-auto', className)} {...props} />;
}

const badgeVariants = cva('inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[0.6875rem] font-medium', {
  variants: {
    tone: {
      neutral: 'bg-surface-sunken text-muted',
      accent: 'bg-accent-soft text-accent',
      danger: 'bg-danger-soft text-danger',
      outline: 'border border-border text-muted',
    },
  },
  defaultVariants: { tone: 'neutral' },
});

export function Badge({
  className,
  tone,
  ...props
}: React.ComponentProps<'span'> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ tone }), className)} {...props} />;
}

export function Separator({
  className,
  orientation = 'horizontal',
  ...props
}: React.ComponentProps<typeof SeparatorPrimitive.Root>) {
  return (
    <SeparatorPrimitive.Root
      orientation={orientation}
      className={cn('shrink-0 bg-border', orientation === 'horizontal' ? 'h-px w-full' : 'h-full w-px', className)}
      {...props}
    />
  );
}

/** Shown wherever a pane has nothing to display yet. */
export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: React.ReactNode;
  title: string;
  description?: React.ReactNode;
  action?: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('flex h-full flex-col items-center justify-center gap-3 p-8 text-center', className)}>
      {icon ? <div className='text-border-strong [&_svg]:size-8'>{icon}</div> : null}
      <div className='space-y-1'>
        <p className='text-sm font-medium'>{title}</p>
        {description ? <p className='max-w-sm text-xs leading-relaxed text-muted'>{description}</p> : null}
      </div>
      {action}
    </div>
  );
}

/** A single row in one of the file or entry lists. */
export function ListRow({ active, className, ...props }: React.ComponentProps<'button'> & { active?: boolean }) {
  return (
    <button
      type='button'
      className={cn(
        'flex w-full items-center gap-2 border-l-2 px-3 py-1.5 text-left text-sm transition-colors',
        active
          ? 'border-l-accent bg-accent-soft/60 text-foreground'
          : 'border-l-transparent text-muted hover:bg-surface-raised hover:text-foreground',
        className,
      )}
      {...props}
    />
  );
}
