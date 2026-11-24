import { Ellipsis } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import { useRef, type PointerEvent, type ReactElement } from "react";
import { Button } from "../shadcn/button";
import { Button as WorkspaceButton } from "../ui";
import { actionItemClass, actionMenuClass, ActionsContextMenu } from "./ActionsContextMenu";

export type CollectionActionsProps = {
  name: string;
  onRename: () => void;
  onDelete: () => void;
  moveEarlier?: () => void;
  moveLater?: () => void;
};

export type CollectionMove = (source: string, destination: string, after: boolean) => void;

export function useCollectionDrag(name: string, onMove: CollectionMove, inGrid = false) {
  const drag = useRef<{ pointer: number; x: number; y: number; moved: boolean } | null>(null);
  const suppressClick = useRef(false);

  function onPointerDown(event: PointerEvent<HTMLButtonElement>) {
    if (!event.isPrimary || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    suppressClick.current = false;
    drag.current = { pointer: event.pointerId, x: event.clientX, y: event.clientY, moved: false };
  }

  function onPointerMove(event: PointerEvent<HTMLButtonElement>) {
    const start = drag.current;
    if (!start || start.pointer !== event.pointerId || start.moved || !(event.buttons & 1)) return;
    if (Math.hypot(event.clientX - start.x, event.clientY - start.y) < 5) return;
    start.moved = true;
    suppressClick.current = true;
  }

  function onPointerUp(event: PointerEvent<HTMLButtonElement>) {
    const start = drag.current;
    drag.current = null;
    if (!start || start.pointer !== event.pointerId) return;
    if (!start.moved && Math.hypot(event.clientX - start.x, event.clientY - start.y) < 5) return;
    suppressClick.current = true;
    const target = document.elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>("[data-collection-drag-target]");
    const destination = target?.dataset.collectionDragTarget;
    if (!target || !destination || destination === name) return;
    const bounds = target.getBoundingClientRect();
    const after = inGrid
      ? event.clientX >= bounds.left + bounds.width / 2
      : event.clientY >= bounds.top + bounds.height / 2;
    onMove(name, destination, after);
  }

  return {
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel: () => { drag.current = null; },
    onClickCapture: (event: React.MouseEvent) => {
      if (!suppressClick.current || event.detail === 0) return;
      suppressClick.current = false;
      event.stopPropagation();
      event.preventDefault();
    },
    "data-collection-drag-target": name,
  };
}

export function DraggableCollectionButton({
  name, onMove, ...props
}: React.ComponentProps<typeof WorkspaceButton> & { name: string; onMove: CollectionMove }) {
  const drag = useCollectionDrag(name, onMove, true);
  return <WorkspaceButton {...props} {...drag} />;
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
