import { observer } from "mobx-react-lite";
import { api } from "../../lib/api";
import {
derived,
state,
tick,
useController,
type Lifecycle
} from "../controller";
import { Button } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
type Props = {
  onclose: () => void;
  prepare: (target: string) => Promise<void>;
  hasRepo?: boolean;
};
function createView(lifecycle: Lifecycle<Props>) {
  const onclose = derived(() => lifecycle.props().onclose);
  const prepare = derived(() => lifecycle.props().prepare);
  const hasRepo = derived(() => lifecycle.props().hasRepo ?? false);
  let step = state(0),
    busy = state(false),
    saving = state(false),
    error = state("");
  let heading: HTMLHeadingElement;
  let alive = true,
    frame = 0;
  let ring = state({ left: 0, top: 0, width: 0, height: 0 });
  let card = state({ left: 20, top: 20 });
  const steps = derived(() => [
    {
      title: "Add your first repository.",
      body: "Choose Create, Add from folder or Clone. Select where the repository lives on your computer.",
      target: "add",
    },
    {
      title: "Find a project quickly.",
      body: "Search by name, branch, tag or collection. Ctrl+K brings you straight here.",
      target: "search",
    },
    ...(hasRepo.value
      ? [
          {
            title: "Review and commit.",
            body: "Open a file to review its diff. Write a message and commit the changes you want to keep.",
            target: "commit",
          },
        ]
      : []),
  ]);
  const current = derived(() => steps.value[step.value]);
  function position() {
    const el = document.querySelector<HTMLElement>(
      '[data-tour="' + current.value.target + '"]',
    );
    if (!el) return;
    const b = el.getBoundingClientRect();
    ring.value = {
      left: Math.max(6, b.left - 7),
      top: Math.max(6, b.top - 7),
      width: Math.min(b.width + 14, innerWidth - 12),
      height: b.height + 14,
    };
    const width = Math.min(370, innerWidth - 32);
    const below = b.bottom + 24;
    card.value = {
      left: Math.max(16, Math.min(b.left, innerWidth - width - 16)),
      top: below + 245 < innerHeight ? below : Math.max(16, b.top - 265),
    };
  }
  async function move(next: number) {
    if (busy.value) return;
    busy.value = true;
    step.value = next;
    error.value = "";
    try {
      await prepare.value(current.value.target);
      await tick();
      if (!alive) return;
      document
        .querySelector<HTMLElement>(
          '[data-tour="' + current.value.target + '"]',
        )
        ?.scrollIntoView({ block: "center", behavior: "instant" });
      position();
      heading?.focus();
    } catch {
      error.value = "This part of the tour is unavailable. You can skip it.";
    } finally {
      busy.value = false;
    }
  }
  async function finish() {
    if (saving.value) return;
    saving.value = true;
    error.value = "";
    try {
      await api.finishWalkthrough();
      onclose.value();
    } catch {
      error.value = "Could not save your preference. Try again.";
    } finally {
      saving.value = false;
    }
  }
  lifecycle.mount(() => {
    document.body.classList.add("tour-active");
    void move(0);
    const reposition = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(position);
    };
    window.addEventListener("resize", reposition);
    document.addEventListener("scroll", reposition, true);
    return () => {
      alive = false;
      cancelAnimationFrame(frame);
      window.removeEventListener("resize", reposition);
      document.removeEventListener("scroll", reposition, true);
      document.body.classList.remove("tour-active");
    };
  });
  return () => (
    <>
      <Modal
        label={"Reponiq walkthrough"}
        onclose={finish}
        canClose={!saving.value}
        walkthrough={true}
      >
        <div
          aria-hidden={"true"}
          className={cn("tour-ring")}
          style={{
            left: ring.value.left + "px",
            top: ring.value.top + "px",
            width: ring.value.width + "px",
            height: ring.value.height + "px",
          }}
        />
        <section
          className={cn("tour-card")}
          style={{ left: card.value.left + "px", top: card.value.top + "px" }}
        >
          <Button
            aria-label={"Close dialog"}
            title={"Close dialog"}
            disabled={saving.value}
            onClick={finish}
            className={cn("dialog-close")}
          >
            {"×"}
          </Button>
          <p aria-live={"polite"} className={cn("tour-progress")}>
            {step.value + 1}
            {" / "}
            {steps.value.length}
          </p>
          <h2
            ref={(node) => {
              if (node) heading = node;
            }}
            tabIndex={-1}
          >
            {current.value.title}
          </h2>
          <p>{current.value.body}</p>
          {error.value ? (
            <>
              <p role={"alert"} className={cn("connection-error")}>
                {error.value}
              </p>
            </>
          ) : null}
          <div className={cn("tour-actions")}>
            <Button
              disabled={saving.value}
              onClick={finish}
              className={cn("text-action")}
            >
              {"Skip tour"}
            </Button>
            <div>
              {step.value > 0 ? (
                <>
                  <Button
                    variant={"secondary"}
                    disabled={busy.value || saving.value}
                    onClick={() => move(step.value - 1)}
                  >
                    {"Back"}
                  </Button>
                </>
              ) : null}
              <Button
                variant={"primary"}
                disabled={busy.value || saving.value}
                onClick={() =>
                  step.value === steps.value.length - 1
                    ? finish()
                    : move(step.value + 1)
                }
              >
                {step.value === steps.value.length - 1 ? "Get started" : "Next"}
              </Button>
            </div>
          </div>
        </section>
      </Modal>
    </>
  );
}
export default observer(function Walkthrough(props: Props) {
  return useController(createView, props);
});
