import { X } from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import {
derived,
state,
tick,
useController,
type Lifecycle
} from "../controller";
import { Badge,Button,Input,Label } from "../ui";
import { cn } from "../utils";
type BaseProps = {
  label: string;
  items?: string[];
  maxItems?: number;
  maxLength?: number;
};
type Props = BaseProps & {
  onItemsChange?: (value: NonNullable<BaseProps["items"]>) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const label = derived(() => lifecycle.props().label);
  const items = lifecycle.bound("items", [], "onItemsChange");
  const maxItems = derived(() => lifecycle.props().maxItems ?? 20);
  const maxLength = derived(() => lifecycle.props().maxLength ?? 50);
  let draft = state("");
  let input = state<HTMLInputElement>();
  const id = lifecycle.id;
  function add() {
    const value = draft.value.trim();
    if (
      value &&
      items.value.length < maxItems.value &&
      !items.value.some(
        (item) => item.toLocaleLowerCase() === value.toLocaleLowerCase(),
      )
    )
      items.value = [...items.value, value];
    draft.value = "";
  }
  async function remove(item: string) {
    items.value = items.value.filter((value) => value !== item);
    await tick();
    input.value?.focus();
  }
  return () => (
    <>
      <div className={cn("token-field")}>
        <Label htmlFor={id}>{label.value}</Label>
        <div className={cn("token-input")}>
          <div className={cn("tokens")}>
            {(() => {
              const entries = items.value;
              return entries.map((item, index) => (
                <Fragment key={index}>
                  <Badge className={cn("token-chip")}>
                    <span>{item}</span>
                    <Button
                      type={"button"}
                      aria-label={
                        "Remove " + label.value.toLowerCase() + " " + item
                      }
                      onClick={() => remove(item)}
                    >
                      <X size={13} />
                    </Button>
                  </Badge>
                </Fragment>
              ));
            })()}
          </div>
          <Input
            id={id}
            ref={(node) => {
              if (node) input.value = node;
            }}
            aria-label={label.value}
            placeholder={
              items.value.length >= maxItems.value ? "" : "Type and press Enter"
            }
            maxLength={maxLength.value}
            disabled={items.value.length >= maxItems.value}
            value={draft.value}
            onValueChange={(next) => {
              draft.value = next;
            }}
            onKeyDown={(event) => {
              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
                event.preventDefault();
                add();
              }
            }}
            onBlur={add}
          />
        </div>
      </div>
    </>
  );
}
export default observer(function TokenInput(props: Props) {
  return useController(createView, props);
});
