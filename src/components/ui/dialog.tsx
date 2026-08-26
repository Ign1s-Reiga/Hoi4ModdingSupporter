'use client';

import * as React from 'react';
import * as DialogPrimitive from '@radix-ui/react-dialog';
import { X } from 'lucide-react';

import { cn } from '@/lib/utils';

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogClose = DialogPrimitive.Close;

export function DialogContent({
  className,
  children,
  title,
  description,
  ...props
}: React.ComponentProps<typeof DialogPrimitive.Content> & {
  title: string;
  description?: string;
}) {
  return (
    <DialogPrimitive.Portal>
      <DialogPrimitive.Overlay className='fixed inset-0 z-50 bg-black/55 data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0' />
      <DialogPrimitive.Content
        className={cn(
          'fixed left-1/2 top-1/2 z-50 w-full max-w-lg -translate-x-1/2 -translate-y-1/2 rounded-lg border border-border bg-surface shadow-xl',
          'data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=open]:zoom-in-95',
          className,
        )}
        {...props}
      >
        <div className='flex items-start gap-3 border-b border-border px-4 py-3'>
          <div className='min-w-0 flex-1'>
            <DialogPrimitive.Title className='text-sm font-medium'>{title}</DialogPrimitive.Title>
            {description ? (
              <DialogPrimitive.Description className='mt-0.5 text-xs text-muted'>
                {description}
              </DialogPrimitive.Description>
            ) : null}
          </div>
          <DialogPrimitive.Close className='rounded-sm p-1 text-muted transition-colors hover:bg-surface-sunken hover:text-foreground'>
            <X className='size-4' />
            <span className='sr-only'>Close</span>
          </DialogPrimitive.Close>
        </div>
        {children}
      </DialogPrimitive.Content>
    </DialogPrimitive.Portal>
  );
}

export function DialogBody({ className, ...props }: React.ComponentProps<'div'>) {
  return <div className={cn('max-h-[60vh] overflow-auto p-4', className)} {...props} />;
}

export function DialogFooter({ className, ...props }: React.ComponentProps<'div'>) {
  return (
    <div
      className={cn(
        'flex items-center justify-end gap-2 border-t border-border bg-surface-raised px-4 py-3',
        className,
      )}
      {...props}
    />
  );
}
