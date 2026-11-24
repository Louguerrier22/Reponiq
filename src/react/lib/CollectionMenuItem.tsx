import { ChevronRight, Folder } from "lucide-react";
import { useState } from "react";
import { CollectionContextMenu, useCollectionDrag, type CollectionActionsProps, type CollectionMove } from "./CollectionActions";
import type { Repo } from "../../lib/api";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../shadcn/collapsible";
import {
  SidebarMenuAction,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  useSidebar,
} from "../shadcn/sidebar";
import { Button } from "../ui";

const pageSize = 5;

export default function CollectionMenuItem({ name, repositories, active, selectedRepo, onOpen, onRepository, actions, onMove }: {
  name: string;
  repositories: Repo[];
  active: boolean;
  selectedRepo: number | null;
  onOpen: () => void;
  onRepository: (id: number) => void;
  actions: CollectionActionsProps;
  onMove: CollectionMove;
}) {
  const [open, setOpen] = useState(false);
  const [limit, setLimit] = useState(pageSize);
  const { isMobile, setOpenMobile } = useSidebar();
  const drag = useCollectionDrag(name, onMove);
  const toggleLabel = `${open ? "Collapse" : "Expand"} ${name}`;

  return (
    <Collapsible asChild open={open} onOpenChange={(next) => {
      setOpen(next);
      if (!next) setLimit(pageSize);
    }}>
      <SidebarMenuItem className="collection-menu-item" data-collection-item={name}>
        <CollectionContextMenu {...actions}>
          <SidebarMenuButton asChild isActive={active} tooltip={name}>
            <Button
              {...drag}
              title={name}
              className={active ? "active" : undefined}
              aria-current={active ? "page" : undefined}
              onClick={() => {
                onOpen();
                if (isMobile) setOpenMobile(false);
              }}
            >
              <Folder size={17} aria-hidden="true" />
              <span>{name}</span>
            </Button>
          </SidebarMenuButton>
        </CollectionContextMenu>
        <CollapsibleTrigger asChild>
          <SidebarMenuAction type="button" className="collection-menu-expand" aria-label={toggleLabel} title={toggleLabel}>
            <ChevronRight size={15} aria-hidden="true" />
          </SidebarMenuAction>
        </CollapsibleTrigger>
        <CollapsibleContent asChild>
          <SidebarMenuSub className="collection-menu-projects">
            {repositories.slice(0, limit).map((repo) => (
              <SidebarMenuSubItem key={repo.id}>
                <SidebarMenuSubButton asChild isActive={selectedRepo === repo.id} size="sm">
                  <button type="button" title={repo.name} aria-current={selectedRepo === repo.id ? "page" : undefined} onClick={() => {
                    onRepository(repo.id);
                    if (isMobile) setOpenMobile(false);
                  }}>
                    <span>{repo.name}</span>
                  </button>
                </SidebarMenuSubButton>
              </SidebarMenuSubItem>
            ))}
            {repositories.length > limit && (
              <SidebarMenuSubItem>
                <SidebarMenuSubButton asChild size="sm">
                  <button type="button" onClick={() => setLimit((count) => count + pageSize)}>
                    Show more
                  </button>
                </SidebarMenuSubButton>
              </SidebarMenuSubItem>
            )}
          </SidebarMenuSub>
        </CollapsibleContent>
      </SidebarMenuItem>
    </Collapsible>
  );
}
