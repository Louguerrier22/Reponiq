import {
  type ComponentProps,
  type PropsWithChildren,
  type ReactNode,
} from "react";
import { Alert as ShadcnAlert } from "./shadcn/alert";
import { Badge as ShadcnBadge } from "./shadcn/badge";
import { Button as ShadcnButton } from "./shadcn/button";
import { Card as ShadcnCard } from "./shadcn/card";
import { Checkbox as ShadcnCheckbox } from "./shadcn/checkbox";
import {
  CollapsibleContent,
  CollapsibleTrigger,
  Collapsible as ShadcnCollapsible,
} from "./shadcn/collapsible";
import { EmptyTitle, Empty as ShadcnEmpty } from "./shadcn/empty";
import { Field as ShadcnField } from "./shadcn/field";
import { Input as ShadcnInput } from "./shadcn/input";
import { Label } from "./shadcn/label";
import { NativeSelect as ShadcnNativeSelect } from "./shadcn/native-select";
import { Progress as ShadcnProgress } from "./shadcn/progress";
import {
  Sidebar as ShadcnSidebar,
  SidebarMenuButton as ShadcnSidebarMenuButton,
  SidebarContent,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
  useSidebar,
} from "./shadcn/sidebar";
import { Toaster as ShadcnToaster } from "./shadcn/sonner";
import { Switch } from "./shadcn/switch";
import {
  TabsList as ShadcnTabsList,
  TabsContent,
  Tabs as TabsRoot,
  TabsTrigger,
} from "./shadcn/tabs";
import { Textarea as ShadcnTextarea } from "./shadcn/textarea";
import {
  ToggleGroup as ShadcnToggleGroup,
  ToggleGroupItem,
} from "./shadcn/toggle-group";
import { cn } from "./utils";

export { Badge, Label, SidebarTrigger, Switch, TabsTrigger };
const Badge = ShadcnBadge;
export const Tabs = { Root: TabsRoot, Content: TabsContent };
export const TabsList = ShadcnTabsList;

type ButtonProps = Omit<ComponentProps<typeof ShadcnButton>, "variant"> & {
  variant?: "default" | "primary" | "secondary" | "ghost" | "destructive";
};
export function Button({
  className,
  type = "button",
  variant = "default",
  ...props
}: ButtonProps) {
  return (
    <ShadcnButton
      type={type}
      variant={
        variant === "primary"
          ? "default"
          : variant === "default"
            ? "ghost"
            : variant
      }
      className={cn(
        variant === "default" && "reponiq-plain-button",
        variant !== "default" && variant,
        className,
      )}
      {...props}
    />
  );
}

export function Card({
  as = "div",
  ...props
}: ComponentProps<"div"> & { as?: "article" | "div" | "section" }) {
  return <ShadcnCard as={as} {...props} />;
}

export function Input({
  onValueChange,
  onChange,
  onInput,
  value,
  ...props
}: Omit<ComponentProps<typeof ShadcnInput>, "value" | "onInput"> & {
  value?: ComponentProps<typeof ShadcnInput>["value"] | null;
  onInput?: ComponentProps<"input">["onChange"];
  onValueChange?: (next: string) => void;
}) {
  return (
    <ShadcnInput
      {...props}
      value={value === null ? "" : value}
      onChange={(event) => {
        onValueChange?.(event.currentTarget.value);
        onInput?.(event);
        onChange?.(event);
      }}
    />
  );
}
export function Textarea({
  onValueChange,
  onChange,
  onInput,
  ...props
}: Omit<ComponentProps<typeof ShadcnTextarea>, "onInput"> & {
  onInput?: ComponentProps<"textarea">["onChange"];
  onValueChange?: (next: string) => void;
}) {
  return (
    <ShadcnTextarea
      {...props}
      onChange={(event) => {
        onValueChange?.(event.currentTarget.value);
        onInput?.(event);
        onChange?.(event);
      }}
    />
  );
}

export function NativeSelect<T extends string | number | boolean | null>({
  value,
  onValueChange,
  onChange,
  ...props
}: Omit<
  ComponentProps<typeof ShadcnNativeSelect>,
  "value" | "onValueChange"
> & { value?: T; onValueChange?: (next: T) => void }) {
  return (
    <ShadcnNativeSelect
      {...props}
      value={value == null ? "" : String(value)}
      onChange={(event) => {
        const select = event.currentTarget;
        const kind = select.selectedOptions[0]?.dataset.optionType;
        onValueChange?.(
          (kind === "null"
            ? null
            : kind === "number"
              ? Number(select.value)
              : kind === "boolean"
                ? select.value === "true"
                : select.value) as T,
        );
        onChange?.(event);
      }}
    />
  );
}

export function Checkbox({
  className,
  onCheckedChange,
  ...props
}: Omit<ComponentProps<typeof ShadcnCheckbox>, "onCheckedChange"> & {
  onCheckedChange?: (checked: boolean) => void;
}) {
  return (
    <ShadcnCheckbox
      className={cn("ui-checkbox", className)}
      {...props}
      onCheckedChange={(value) => onCheckedChange?.(value === true)}
    />
  );
}

export function Collapsible({
  className,
  heading,
  children,
  open,
  onOpenChange,
}: PropsWithChildren<{
  className?: string;
  heading: () => ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}>) {
  return (
    <ShadcnCollapsible
      className={className}
      open={open}
      onOpenChange={onOpenChange}
    >
      <CollapsibleTrigger className="ui-disclosure-trigger">
        {heading()}
      </CollapsibleTrigger>
      <CollapsibleContent className="ui-disclosure-content">
        {children}
      </CollapsibleContent>
    </ShadcnCollapsible>
  );
}

type ToggleOption = { value: string; label: string };
export function ToggleGroup({
  label,
  options,
  value,
  disabled,
  className,
  onchange,
  item,
}: {
  label: string;
  options: ToggleOption[];
  value: string;
  disabled?: boolean;
  className?: string;
  onchange: (value: string) => void;
  item?: (option: ToggleOption) => ReactNode;
}) {
  return (
    <ShadcnToggleGroup
      type="single"
      aria-label={label}
      size="sm"
      spacing={1}
      className={cn("flex-wrap", className)}
      value={value}
      disabled={disabled}
      onValueChange={(next) => {
        if (next) onchange(next);
      }}
    >
      {options.map((option) => (
        <ToggleGroupItem
          key={option.value}
          value={option.value}
          aria-label={option.label}
          title={option.label}
          className="text-muted-foreground data-[state=on]:bg-primary/15 data-[state=on]:text-primary"
        >
          {item ? item(option) : option.label}
        </ToggleGroupItem>
      ))}
    </ShadcnToggleGroup>
  );
}

export function Sidebar({ children }: PropsWithChildren) {
  return (
    <SidebarProvider className="workspace-sidebar-provider">
      <SidebarTrigger
        className="workspace-sidebar-trigger"
        aria-label="Toggle workspace sidebar"
      />
      <ShadcnSidebar className="workspace-sidebar" collapsible="icon">
        <SidebarContent className="sidebar" aria-label="Workspace navigation">
          {children}
        </SidebarContent>
      </ShadcnSidebar>
    </SidebarProvider>
  );
}

export function SidebarMenuButton({
  active = false,
  ...props
}: ComponentProps<typeof Button> & { active?: boolean }) {
  const { isMobile, setOpenMobile } = useSidebar();
  return (
    <SidebarMenuItem>
      <ShadcnSidebarMenuButton asChild isActive={active} tooltip={props.title}>
        <Button
          aria-current={active ? "page" : undefined}
          {...props}
          onClick={(event) => {
            props.onClick?.(event);
            if (isMobile && !event.defaultPrevented) setOpenMobile(false);
          }}
          className={cn(active && "active", props.className)}
        />
      </ShadcnSidebarMenuButton>
    </SidebarMenuItem>
  );
}

export function Field({
  id,
  label,
  children,
}: PropsWithChildren<{ id: string; label: string }>) {
  return (
    <ShadcnField className="ui-field">
      <Label htmlFor={id}>{label}</Label>
      {children}
    </ShadcnField>
  );
}
export function Progress({
  value,
  max,
  label,
}: {
  value: number;
  max: number;
  label: string;
}) {
  return <ShadcnProgress aria-label={label} value={value} max={max} />;
}
export function Alert({
  className,
  urgent = true,
  children,
}: PropsWithChildren<{ className?: string; urgent?: boolean }>) {
  return (
    <ShadcnAlert
      role={urgent ? "alert" : "status"}
      className={cn("alert", className)}
    >
      {children}
    </ShadcnAlert>
  );
}
export function Empty({
  title,
  children,
}: PropsWithChildren<{ title: string }>) {
  return (
    <ShadcnEmpty className="empty-block">
      <EmptyTitle>{title}</EmptyTitle>
      {children}
    </ShadcnEmpty>
  );
}

export function Toaster({ dark }: { dark: boolean }) {
  return (
    <ShadcnToaster
      theme={dark ? "dark" : "light"}
      position="bottom-right"
      duration={5000}
      visibleToasts={1}
      closeButton
      className="reponiq-toaster"
    />
  );
}
