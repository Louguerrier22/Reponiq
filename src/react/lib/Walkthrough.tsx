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
    title: "Open a repository",
    body: "Use Add repository to create, clone, or add an existing folder. Use Import to add several at once.",
    target: "add",
  },
  {
    title: "Find and organize your work",
    body: "Search by name, branch, tag, or collection. Pin frequent repositories or switch between grid and row views.",
    target: "search",
  },
  {
    title: "Group related repositories",
    body: "Use the plus beside Collections, then choose a repository and a collection.",
    target: "collections",
  },
  {
    title: "Work on a branch",
    body: "Open the branch control to create or switch branches. Use Stash changes to set work aside, then Saved changes to bring it back.",
    target: "branches",
  },
  {
    title: "Fetch, pull, and push",
    body: "Fetch checks for remote changes. Pull brings them into your current branch. Push sends your commits to the remote.",
    target: "sync",
  },
  {
    title: "Review and commit",
    body: "Open a file to review its diff. With no files checked, Commit uses all changes. Switch between Exclude checked and Include checked to choose files.",
    target: "commit",
  },
  {
    title: "Look back at a change",
    body: "Open History to inspect earlier commits and their diffs. Open Activity history in the sidebar for workspace events.",
    target: "history",
  },
  {
    title: "Connect a GitHub account",
    body: "Open Settings → GitHub accounts to connect or manage an account.",
    target: "connect",
  },
  {
    title: "Review pull requests and checks",
    body: "For a connected GitHub repository, open Pull requests to review changes and checks. Open Workflows to see runs.",
    target: "pulls",
  },
  {
    title: "Choose what fetches on launch",
    body: "Set a workspace default and choose repositories in Settings → General → Startup. Use Fetch on launch on a repository page to change its preference.",
    target: "startup",
  },
  {
    title: "Install an app update",
    body: "Open App updates from the sidebar. When an update is ready, you can restart from there.",
    target: "updates",
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
          <div aria-hidden="true" className="tour-ring" style={ring.value} />
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
