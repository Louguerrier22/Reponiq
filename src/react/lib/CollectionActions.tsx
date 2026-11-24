import { Ellipsis } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import type { DragEvent, ReactElement } from "react";
import { Button } from "../shadcn/button";
import { actionItemClass, actionMenuClass, ActionsContextMenu } from "./ActionsContextMenu";

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

function collectionItems({ onRename, onDelete, moveEarlier, moveLater }: CollectionActionsProps) {
  return [
    { label: "Rename", select: onRename },
    { label: "Delete", select: onDelete },
    { label: "Move earlier", select: moveEarlier },
    { label: "Move later", select: moveLater },
  ];
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
        <DropdownMenu.Content className={actionMenuClass} align="start" sideOffset={4}>
          {collectionItems(props).map(({ label, select }) => (
            <DropdownMenu.Item key={label} className={actionItemClass} disabled={!select} onSelect={select}>
              {label}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

export function CollectionContextMenu({ children, ...props }: CollectionActionsProps & { children: ReactElement }) {
  return <ActionsContextMenu actions={collectionItems(props)}>{children}</ActionsContextMenu>;
}
