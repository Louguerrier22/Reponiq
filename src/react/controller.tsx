import {
autorun,
computed,
configure,
observable,
toJS,
untracked,
} from "mobx";
import {
Component,
useEffect,
useId,
useLayoutEffect,
useState,
type ReactNode,
} from "react";

configure({ enforceActions: "never" });

export const untrack = untracked;
export const snapshot = toJS;
export const tick = () =>
  new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

export function state<T>(initial: T): { value: T };
export function state<T = undefined>(): { value: T | undefined };
export function state<T>(initial?: T) {
  const value = observable.box(initial);
  return {
    get value() {
      return value.get();
    },
    set value(next) {
      value.set(next);
    },
  };
}

export function derived<T>(compute: () => T) {
  const value = computed(compute);
  return {
    get value() {
      return value.get();
    },
  };
}

type Cleanup = () => void;
type Effect = () => void | Cleanup;

export class Lifecycle<Props extends object> {
  private readonly current;
  private readonly effects: Effect[] = [];
  private readonly mounts: Effect[] = [];
  private readonly destroys: Cleanup[] = [];

  constructor(
    props: Props,
    readonly id: string,
  ) {
    this.current = observable.box(props, { deep: false });
  }

  props = () => this.current.get();
  setProps = (props: Props) => this.current.set(props);
  effect = (fn: Effect) => {
    this.effects.push(fn);
  };
  mount = (fn: Effect) => {
    this.mounts.push(fn);
  };
  destroy = (fn: Cleanup) => {
    this.destroys.push(fn);
  };

  bound<Key extends keyof Props>(
    key: Key,
    initial: NonNullable<Props[Key]>,
    callback: keyof Props,
  ) {
    type Value = NonNullable<Props[Key]>;
    const local = state<Value>(initial);
    const props = this.props;
    return {
      get value() {
        return (props()[key] ?? local.value) as Value;
      },
      set value(value: Value) {
        const onChange = props()[callback] as
          | ((value: Value) => void)
          | undefined;
        if (onChange) onChange(value);
        else local.value = value;
      },
    };
  }

  start() {
    const cleanups = this.effects.map((effect) => {
      let cleanup: void | Cleanup;
      const stop = autorun(() => {
        if (cleanup) untracked(cleanup);
        cleanup = effect();
      });
      return () => {
        stop();
        cleanup?.();
      };
    });
    const mounts = this.mounts.map((mount) => untracked(mount));
    return () => {
      for (const cleanup of cleanups) cleanup();
      for (const cleanup of mounts) cleanup?.();
      for (const destroy of this.destroys) destroy();
    };
  }
}

export function useController<Props extends object>(
  create: (lifecycle: Lifecycle<Props>) => () => ReactNode,
  props: Props,
) {
  const id = useId();
  const [{ lifecycle, render }] = useState(() => {
    const lifecycle = new Lifecycle(props, id);
    return { lifecycle, render: create(lifecycle) };
  });
  useLayoutEffect(() => {
    lifecycle.setProps(props);
  }, [lifecycle, props]);
  useLayoutEffect(() => lifecycle.start(), [lifecycle]);
  return render();
}

export function WindowEvents({
  onKeyDown,
}: {
  onKeyDown: (event: KeyboardEvent) => void;
}) {
  useEffect(() => {
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onKeyDown]);
  return null;
}

export class LoadBoundary extends Component<
  { children: ReactNode },
  { error: string }
> {
  state = { error: "" };
  static getDerivedStateFromError(error: unknown) {
    return { error: String(error) };
  }
  render() {
    return this.state.error ? (
      <div className="alert error" role="alert">
        {this.state.error}
      </div>
    ) : (
      this.props.children
    );
  }
}

export function SelectOption({
  value,
  ...props
}: Omit<React.ComponentProps<"option">, "value"> & {
  value?: string | boolean | number | null;
}) {
  return (
    <option
      value={value == null ? "" : String(value)}
      data-option-type={value === null ? "null" : typeof value}
      {...props}
    />
  );
}
