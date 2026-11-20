import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import type { Branch,RepoDetails } from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Input,Label } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
type Props = {
  details: RepoDetails;
  busy: boolean;
  error: string;
  recentBranches?: string[];
  defaultBranch?: string | null;
  onclose: () => void;
  onswitch: (branch: Branch, strategy: string) => Promise<boolean>;
  onfetch: () => Promise<boolean>;
  oncreate: () => void;
  onopenworktree: (reference: string) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const details = derived(() => lifecycle.props().details);
  const busy = derived(() => lifecycle.props().busy);
  const error = derived(() => lifecycle.props().error);
  const recentBranches = derived(() => lifecycle.props().recentBranches ?? []);
  const defaultBranch = derived(() => lifecycle.props().defaultBranch ?? null);
  const onclose = derived(() => lifecycle.props().onclose);
  const onswitch = derived(() => lifecycle.props().onswitch);
  const onfetch = derived(() => lifecycle.props().onfetch);
  const oncreate = derived(() => lifecycle.props().oncreate);
  const onopenworktree = derived(() => lifecycle.props().onopenworktree);
  let query = state("");
  let target = state<Branch | null>(null);
  let visibleCount = state(60);
  const rank = (branch: Branch) => {
    if (branch.remote) return 100;
    if (branch.name === details.value.git.branch) return -2;
    if (branch.name === defaultBranch.value) return -1;
    const recent = recentBranches.value.indexOf(branch.name);
    return recent === -1 ? 90 : recent;
  };
  let filtered = derived(() =>
    details.value.branches
      .filter((branch) =>
        (branch.remote ? branch.remote + "/" + branch.name : branch.name)
          .toLocaleLowerCase()
          .includes(query.value.trim().toLocaleLowerCase()),
      )
      .sort((a, b) => rank(a) - rank(b)),
  );
  const blocked = derived(
    () =>
      !!details.value.git.operation || details.value.git.conflicts.length > 0,
  );
  async function choose(branch: Branch) {
    if (details.value.git.dirty) target.value = branch;
    else if (await onswitch.value(branch, "clean")) onclose.value();
  }
  async function switchTarget(strategy: string) {
    if (target.value && (await onswitch.value(target.value, strategy)))
      onclose.value();
  }
  return () => (
    <>
      <Modal
        label={"Switch branch"}
        canClose={!busy.value}
        onclose={onclose.value}
      >
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
        {busy.value ? (
          <>
            <p role={"status"}>{"Waiting for Git…"}</p>
          </>
        ) : null}
        {target.value ? (
          <>
            <h2>
              {"Switch to "}
              {target.value.remote ? target.value.remote + "/" : ""}
              {target.value.name}
              {"?"}
            </h2>
            <p>
              {details.value.git.dirty}
              {" changed · "}
              {details.value.git.branch}
            </p>
            <div className={cn("branch-decisions")}>
              <Button
                variant={"primary"}
                disabled={
                  busy.value ||
                  blocked.value ||
                  !details.value.git.has_head ||
                  details.value.git.branch === "(detached)"
                }
                onClick={() => switchTarget("stash")}
              >
                {"Stash and switch"}
              </Button>
              <Button
                variant={"secondary"}
                disabled={busy.value || blocked.value}
                onClick={() => switchTarget("bring")}
              >
                {"Switch with changes"}
              </Button>
              <Button
                disabled={busy.value}
                onClick={() => (target.value = null)}
                className={cn("text-action")}
              >
                {"Back"}
              </Button>
            </div>
          </>
        ) : (
          <>
            <h2>{"Switch branch"}</h2>
            <Label htmlFor={"branch-search"}>{"Find a branch"}</Label>
            <Input
              id={"branch-search"}
              type={"search"}
              placeholder={"Search local and fetched branches"}
              value={query.value}
              onValueChange={(next) => {
                query.value = next;
              }}
              onInput={() => (visibleCount.value = 60)}
            />
            {blocked.value ? (
              <>
                <p>
                  {
                    "Resolve conflicts or finish the current Git operation before switching."
                  }
                </p>
              </>
            ) : null}
            <div className={cn("branch-results")}>
              {(() => {
                const entries = filtered.value.slice(0, visibleCount.value);
                return entries.length === 0 ? (
                  <>
                    <p>{"No matching branches"}</p>
                  </>
                ) : (
                  entries.map((branch, index) => (
                    <Fragment key={branch.reference}>
                      {(() => {
                        const current =
                          !branch.remote &&
                          branch.name === details.value.git.branch;
                        return (
                          <>
                            <div className={cn("quick-repo-row")}>
                              <Button
                                aria-current={current ? "true" : undefined}
                                disabled={
                                  busy.value ||
                                  current ||
                                  blocked.value ||
                                  branch.in_worktree
                                }
                                onClick={() => choose(branch)}
                                className={cn([
                                  "commit-choice",
                                  { "is-selected": current },
                                ])}
                              >
                                <span>
                                  <strong>
                                    {branch.remote ? branch.remote + "/" : ""}
                                    {branch.name}
                                  </strong>
                                  <small>
                                    {current
                                      ? "Current branch"
                                      : branch.remote
                                        ? "Remote branch"
                                        : branch.in_worktree
                                          ? "Open in another worktree"
                                          : branch.name === defaultBranch.value
                                            ? "Default branch"
                                            : "Local branch"}
                                  </small>
                                </span>
                              </Button>
                              {!current && branch.in_worktree ? (
                                <>
                                  <Button
                                    variant={"secondary"}
                                    disabled={busy.value}
                                    onClick={() =>
                                      onopenworktree.value(branch.reference)
                                    }
                                  >
                                    {"Open worktree"}
                                  </Button>
                                </>
                              ) : null}
                            </div>
                          </>
                        );
                      })()}
                    </Fragment>
                  ))
                );
              })()}
              {filtered.value.length > visibleCount.value ? (
                <>
                  <Button
                    variant={"secondary"}
                    onClick={() => (visibleCount.value += 60)}
                  >
                    {"Show more branches"}
                  </Button>
                </>
              ) : null}
            </div>
            <div className={cn("modal-actions")}>
              <Button
                disabled={busy.value}
                onClick={onclose.value}
                className={cn("text-action")}
              >
                {"Close"}
              </Button>
              <Button
                variant={"secondary"}
                disabled={busy.value || !details.value.git.remotes.length}
                onClick={onfetch.value}
              >
                {"Fetch branches"}
              </Button>
              <Button
                variant={"primary"}
                disabled={busy.value || blocked.value}
                onClick={oncreate.value}
              >
                {"Create branch…"}
              </Button>
            </div>
          </>
        )}
      </Modal>
    </>
  );
}
export default observer(function Branches(props: Props) {
  return useController(createView, props);
});
