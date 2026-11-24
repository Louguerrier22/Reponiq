import { ContextMenu } from "radix-ui";
import type { ReactElement } from "react";

export const actionMenuClass = "z-[80] min-w-36 rounded-md border bg-popover p-1 text-popover-foreground shadow-md";
export const actionItemClass = "cursor-default rounded-sm px-2 py-1.5 text-sm outline-none data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground data-[disabled]:opacity-50";

export type Action = { label: string; select?: () => void };

export function ActionsContextMenu({ children, actions }: { children: ReactElement; actions: Action[] }) {
  return (
    <ContextMenu.Root modal={false}>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className={actionMenuClass}>
          {actions.map(({ label, select }) => (
            <ContextMenu.Item key={label} className={actionItemClass} disabled={!select} onSelect={select}>
              {label}
            </ContextMenu.Item>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
