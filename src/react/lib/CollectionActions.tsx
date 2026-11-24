import { Ellipsis } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import { useEffect, useRef, type PointerEvent, type ReactElement } from "react";
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

type DragSlot = { element: HTMLElement; name: string; bounds: DOMRect };
type DragPreview = {
  ghost: HTMLElement;
  scope: HTMLElement;
  scopeBounds: DOMRect;
  slots: DragSlot[];
  source: DragSlot;
  offsetX: number;
  offsetY: number;
  index: number;
};

function startPreview(button: HTMLButtonElement, x: number, y: number, inGrid: boolean) {
  const item = button.closest<HTMLElement>("[data-collection-item]");
  const scope = item?.parentElement;
  if (!item || !scope) return null;
  const slots = Array.from(scope.children).flatMap((element) =>
    element instanceof HTMLElement && element.dataset.collectionItem
      ? [{ element, name: element.dataset.collectionItem, bounds: element.getBoundingClientRect() }]
      : [],
  );
  const index = slots.findIndex((slot) => slot.element === item);
  if (index < 0 || slots.length < 2) return null;

  const visual = inGrid ? item : button;
  const bounds = visual.getBoundingClientRect();
  const ghost = visual.cloneNode(true) as HTMLElement;
  ghost.removeAttribute("id");
  ghost.querySelectorAll("[id]").forEach((node) => node.removeAttribute("id"));
  ghost.setAttribute("aria-hidden", "true");
  ghost.inert = true;
  ghost.classList.add("collection-drag-ghost");
  ghost.style.width = bounds.width + "px";
  ghost.style.height = bounds.height + "px";
  document.body.append(ghost);
  item.dataset.collectionGap = "";
  return {
    ghost, scope, scopeBounds: scope.getBoundingClientRect(), slots,
    source: slots[index], offsetX: x - bounds.left, offsetY: y - bounds.top, index,
  };
}

function positionPreview(preview: DragPreview, x: number, y: number, inGrid: boolean) {
  preview.ghost.style.transform = `translate3d(${x - preview.offsetX}px, ${y - preview.offsetY}px, 0)`;
  const scope = preview.scope.getBoundingClientRect();
  const left = x - (scope.left - preview.scopeBounds.left);
  const top = y - (scope.top - preview.scopeBounds.top);
  let nearest = preview.source;
  let nearestDistance = Infinity;
  for (const slot of preview.slots) {
    const { bounds } = slot;
    const dx = inGrid ? Math.max(bounds.left - left, left - bounds.right, 0) : 0;
    const dy = Math.max(bounds.top - top, top - bounds.bottom, 0);
    const distance = dx * dx + dy * dy;
    if (distance < nearestDistance) {
      nearest = slot;
      nearestDistance = distance;
    }
  }
  const others = preview.slots.filter((slot) => slot !== preview.source);
  const bounds = nearest.bounds;
  const after = inGrid ? left >= bounds.left + bounds.width / 2 : top >= bounds.top + bounds.height / 2;
  const index = nearest === preview.source
    ? preview.slots.indexOf(preview.source)
    : others.indexOf(nearest) + Number(after);
  if (index !== preview.index) {
    preview.index = index;
    others.splice(index, 0, preview.source);
    others.forEach((slot, order) => slot.element.style.setProperty("--collection-drag-order", String(order)));
  }
  return nearest === preview.source ? null : { name: nearest.name, after };
}

export function useCollectionDrag(name: string, onMove: CollectionMove, inGrid = false) {
  const drag = useRef<{
    pointer: number; x: number; y: number; button: HTMLButtonElement; preview: DragPreview | null;
  } | null>(null);
  const suppressClick = useRef(false);

  function clearDrag() {
    const current = drag.current;
    drag.current = null;
    if (!current) return;
    if (current.preview) {
      current.preview.ghost.remove();
      delete current.preview.source.element.dataset.collectionGap;
      current.preview.slots.forEach(({ element }) => element.style.removeProperty("--collection-drag-order"));
    }
    if (current.button.hasPointerCapture(current.pointer)) current.button.releasePointerCapture(current.pointer);
  }

  useEffect(() => clearDrag, []);

  function onPointerDown(event: PointerEvent<HTMLButtonElement>) {
    if (!event.isPrimary || event.button !== 0) return;
    clearDrag();
    event.currentTarget.setPointerCapture(event.pointerId);
    suppressClick.current = false;
    drag.current = {
      pointer: event.pointerId, x: event.clientX, y: event.clientY, button: event.currentTarget, preview: null,
    };
  }

  function updateDrag(event: PointerEvent<HTMLButtonElement>) {
    const current = drag.current;
    if (!current || current.pointer !== event.pointerId) return null;
    if (!current.preview && Math.hypot(event.clientX - current.x, event.clientY - current.y) >= 5) {
      suppressClick.current = true;
      current.preview = startPreview(current.button, current.x, current.y, inGrid);
    }
    return current.preview && positionPreview(current.preview, event.clientX, event.clientY, inGrid);
  }

  return {
    onPointerDown,
    onPointerMove: updateDrag,
    onPointerUp: (event: PointerEvent<HTMLButtonElement>) => {
      if (drag.current?.pointer !== event.pointerId) return;
      const destination = updateDrag(event);
      clearDrag();
      if (destination) onMove(name, destination.name, destination.after);
    },
    onPointerCancel: clearDrag,
    onLostPointerCapture: clearDrag,
    onKeyDown: (event: React.KeyboardEvent) => {
      if (event.key === "Escape" && drag.current) {
        suppressClick.current = true;
        clearDrag();
      }
    },
    onDragStart: (event: React.DragEvent) => event.preventDefault(),
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
