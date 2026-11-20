import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import {
derived,
useController,
type Lifecycle
} from "../controller";
import { cn } from "../utils";
type Props = {
  code: string;
  label?: string;
};
function createView(lifecycle: Lifecycle<Props>) {
  const code = derived(() => lifecycle.props().code);
  const label = derived(() => lifecycle.props().label ?? "Diff");
  const lines = derived(() => code.value.slice(0, 200001).split("\n", 1501));
  const limited = derived(
    () => code.value.length > 200000 || lines.value.length > 1500,
  );
  function tone(line: string) {
    return line.startsWith("@@")
      ? "diff-hunk"
      : line.startsWith("+") && !line.startsWith("+++")
        ? "diff-add"
        : line.startsWith("-") && !line.startsWith("---")
          ? "diff-remove"
          : "diff-context";
  }
  return () => (
    <>
      <div
        role={"region"}
        tabIndex={0}
        aria-label={label.value}
        className={cn("diff-code")}
      >
        <pre>
          <code>
            {(() => {
              const entries = lines.value.slice(0, 1500);
              return entries.map((line, index) => (
                <Fragment key={index}>
                  <span className={cn(tone(line))}>
                    <i aria-hidden={"true"}>{index + 1}</i>
                    {line || " "}
                  </span>
                </Fragment>
              ));
            })()}
          </code>
        </pre>
      </div>
      {limited.value ? (
        <>
          <p className={cn("diff-limit")}>
            {"Preview limited to 1,500 lines / 200,000 characters."}
          </p>
        </>
      ) : null}
    </>
  );
}
export default observer(function DiffView(props: Props) {
  return useController(createView, props);
});
