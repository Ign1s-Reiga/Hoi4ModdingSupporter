"use client";

import Link from "next/link";
import { FolderOpen, Loader2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/panel";
import { useAppStore } from "@/lib/store";

export default function WorkspaceLayout({ children }: { children: React.ReactNode }) {
  const project = useAppStore((state) => state.project);
  const isRestoring = useAppStore((state) => state.isRestoring);

  if (isRestoring) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-sm text-muted">
        <Loader2 className="size-4 animate-spin" />
        Restoring your last project…
      </div>
    );
  }

  if (!project) {
    return (
      <EmptyState
        icon={<FolderOpen />}
        title="No mod project open"
        description="Open a .mod descriptor from the home screen to start editing."
        action={
          <Button variant="primary" asChild>
            <Link href="/">Go to home</Link>
          </Button>
        }
      />
    );
  }

  return <div className="h-full">{children}</div>;
}
