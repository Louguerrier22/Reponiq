import { Ellipsis } from "lucide-react";
import { ContextMenu, DropdownMenu } from "radix-ui";
import type { DragEvent, ReactElement } from "react";
import { Button } from "../shadcn/button";

export type CollectionActionsProps = {
  name: string;
  onRename: () => void;
  onDelete: () => void;
  moveEarlier?: () => void;
  moveLater?: () => void;
};

export type CollectionMove = (source: string, destination: string, after: boolean) => void;

const dragType = "application/x-reponiq-collection";

export function collectionDragStart(event: DragEvent, name: string) {
  event.dataTransfer.effectAllowed = "move";
  event.dataTransfer.setData(dragType, name);
}

export function collectionDropTarget(name: string, onMove: CollectionMove, inGrid = false) {
  return {
    onDragOver(event: DragEvent) {
      if (!event.dataTransfer.types.includes(dragType)) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
    },
    onDrop(event: DragEvent) {
      if (!event.dataTransfer.types.includes(dragType)) return;
      event.preventDefault();
      const source = event.dataTransfer.getData(dragType);
      if (!source || source === name) return;
      const bounds = event.currentTarget.getBoundingClientRect();
      const after = inGrid
        ? event.clientX >= bounds.left + bounds.width / 2
        : event.clientY >= bounds.top + bounds.height / 2;
      onMove(source, name, after);
    },
  };
}

const contentClass = "z-[80] min-w-36 rounded-md border bg-popover p-1 text-popover-foreground shadow-md";
const itemClass = "cursor-default rounded-sm px-2 py-1.5 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:opacity-50";

function ActionItems({ menu, onRename, onDelete, moveEarlier, moveLater }: CollectionActionsProps & {
  menu: typeof ContextMenu | typeof DropdownMenu;
}) {
  const Item = menu.Item;
  return (
    <>
      <Item className={itemClass} onSelect={onRename}>Rename</Item>
      <Item className={itemClass} onSelect={onDelete}>Delete</Item>
      <Item className={itemClass} disabled={!moveEarlier} onSelect={moveEarlier}>Move earlier</Item>
      <Item className={itemClass} disabled={!moveLater} onSelect={moveLater}>Move later</Item>
    </>
  );
}

export function CollectionActions(props: CollectionActionsProps) {
  const label = `Actions for ${props.name}`;
  return (
    <DropdownMenu.Root modal={false}>
      <DropdownMenu.Trigger asChild>
        <Button type="button" variant="ghost" size="icon" aria-label={label} title={label}>
          <Ellipsis size={16} aria-hidden="true" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={contentClass} align="start" sideOffset={4}>
          <ActionItems menu={DropdownMenu} {...props} />
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function CollectionContextMenu({ children, ...props }: CollectionActionsProps & { children: ReactElement }) {
  return (
    <ContextMenu.Root modal={false}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className={contentClass}>
          <ActionItems menu={ContextMenu} {...props} />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
