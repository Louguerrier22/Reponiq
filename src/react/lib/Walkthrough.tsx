import { observer } from "mobx-react-lite";
import { api } from "../../lib/api";
import {
derived,
state,
tick,
useController,
type Lifecycle
} from "../controller";
import { Button, Progress } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
const steps = [
  {
    title: "Add a repository",
    body: "Use Add repository to create, clone, or open a folder. Use Import from… to add several existing repositories.",
    target: "add",
  },
  {
    title: "Choose a branch",
    body: "Select the current branch to switch or create a branch.",
    target: "branches",
  },
  {
    title: "Review and commit",
    body: "Select a file to review its changes. Write a commit message and commit. With no files checked, all changes are included.",
    target: "commit",
  },
  {
    title: "Sync with the remote",
    body: "Fetch checks for updates. Pull brings remote commits into your branch. Push sends your commits to the remote.",
    target: "sync",
  },
];

type Props = {
  onclose: () => void;
  prepare: (target: string) => Promise<void>;
};
function createView(lifecycle: Lifecycle<Props>) {
  const onclose = derived(() => lifecycle.props().onclose);
  const prepare = derived(() => lifecycle.props().prepare);
  let step = state(0),
    busy = state(false),
    saving = state(false),
    error = state("");
  let heading: HTMLHeadingElement;
  let tourCard: HTMLElement | null = null;
  let alive = true,
    frame = 0;
  const ring = state<{ left: number; top: number; width: number; height: number } | null>(null);
  let card = state({ left: 20, top: 20 });
  const current = derived(() => steps[step.value]);
  function position() {
    const el = document.querySelector<HTMLElement>(
      '[data-tour="' + current.value.target + '"]',
    );
    const b = el?.getBoundingClientRect();
    const width = tourCard?.offsetWidth ?? Math.min(370, innerWidth - 32);
    const height = tourCard?.offsetHeight ?? 270;
    if (!b?.width || !b.height) {
      ring.value = null;
      card.value = {
        left: Math.max(16, (innerWidth - width) / 2),
        top: Math.max(16, (innerHeight - height) / 2),
      };
      return;
    }
    ring.value = {
      left: Math.max(6, b.left - 7),
      top: Math.max(6, b.top - 7),
      width: Math.min(b.width + 14, innerWidth - 12),
      height: b.height + 14,
    };
    const below = b.bottom + 24;
    card.value = {
      left: Math.max(16, Math.min(b.left, innerWidth - width - 16)),
      top: below + height + 16 <= innerHeight ? below : Math.max(16, b.top - height - 24),
    };
  }
  async function move(next: number) {
    if (busy.value) return;
    busy.value = true;
    ring.value = null;
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
        {ring.value && (
          <div aria-hidden="true" className="tour-ring" style={{ ...ring.value }} />
        )}
        <section
          ref={(node) => { tourCard = node; }}
          className={cn("tour-card")}
          style={{ left: card.value.left + "px", top: card.value.top + "px" }}
        >
          <div className="setup-progress">
            <span aria-hidden="true">{step.value + 1}/{steps.length}</span>
            <Progress label="Guide progress" value={step.value + 1} max={steps.length} />
          </div>
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
              variant="ghost"
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
                  step.value === steps.length - 1
                    ? finish()
                    : move(step.value + 1)
                }
              >
                {step.value === steps.length - 1 ? "Get started" : "Next"}
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
