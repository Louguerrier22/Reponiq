import { observer } from "mobx-react-lite";
import type { KeyboardEvent as ReactKeyboardEvent } from "react";
import { Fragment } from "react";
import { type Repo,type RepoWorkflow } from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Input,Label } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
import "./QuickSwitch.css";
type Props = {
  repos: Repo[];
  metadata: Map<number, RepoWorkflow>;
  onselect: (repo: Repo) => void;
  onclose: () => void;
  onmove: (repo: Repo, earlier: boolean) => Promise<void>;
  onlocation: (repo: Repo, action: "editor" | "terminal" | "folder") => void;
  canOpen: boolean;
};
function createView(lifecycle: Lifecycle<Props>) {
  const repos = derived(() => lifecycle.props().repos);
  const metadata = derived(() => lifecycle.props().metadata);
  const onselect = derived(() => lifecycle.props().onselect);
  const onclose = derived(() => lifecycle.props().onclose);
  const onmove = derived(() => lifecycle.props().onmove);
  const onlocation = derived(() => lifecycle.props().onlocation);
  const canOpen = derived(() => lifecycle.props().canOpen);
  let query = state(""),
    index = state(0),
    moving = state(false);
  const matching = derived(() =>
    repos.value.filter((repo) =>
      [
        repo.name,
        repo.path,
        repo.branch,
        repo.collection ?? "",
        ...repo.tags,
      ].some((text) =>
        text
          .toLocaleLowerCase()
          .includes(query.value.trim().toLocaleLowerCase()),
      ),
    ),
  );
  const pinned = derived(() => matching.value.filter((repo) => repo.pinned));
  const recent = derived(() =>
    matching.value
      .filter((repo) => !repo.pinned && metadata.value.get(repo.id)?.opened_at)
      .sort(
        (a, b) =>
          (metadata.value.get(b.id)?.opened_at ?? 0) -
          (metadata.value.get(a.id)?.opened_at ?? 0),
      )
      .slice(0, 8),
  );
  const results = derived(() =>
    (query.value.trim()
      ? matching.value
      : [
          ...pinned.value,
          ...recent.value,
          ...matching.value.filter(
            (repo) => !repo.pinned && !recent.value.includes(repo),
          ),
        ]
    ).slice(0, 60),
  );
  const selected = derived(
    () => results.value[index.value] ?? results.value[0],
  );
  function keys(event: ReactKeyboardEvent<HTMLInputElement>) {
    if (event.nativeEvent.isComposing || !results.value.length) return;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      index.value =
        (index.value +
          (event.key === "ArrowDown" ? 1 : -1) +
          results.value.length) %
        results.value.length;
      document
        .getElementById("repo-result-" + results.value[index.value].id)
        ?.scrollIntoView({ block: "nearest" });
    } else if (event.key === "Enter" && selected.value) {
      event.preventDefault();
      onselect.value(selected.value);
    }
  }
  async function move(repo: Repo, earlier: boolean) {
    moving.value = true;
    try {
      await onmove.value(repo, earlier);
    } finally {
      moving.value = false;
    }
  }
  return () => (
    <>
      <Modal label={"Switch repository"} onclose={onclose.value}>
        <h2>{"Switch repository"}</h2>
        <Label htmlFor={"quick-repo-search"}>{"Find a repository"}</Label>
        <Input
          id={"quick-repo-search"}
          type={"search"}
          value={query.value}
          placeholder={"Name, folder, branch, or tag"}
          onValueChange={(next) => {
            query.value = next;
          }}
          onInput={() => (index.value = 0)}
          onKeyDown={keys}
        />
        <span role={"status"} className={cn("screen-reader-status")}>
          {selected.value
            ? `${selected.value.name}, ${index.value + 1} of ${results.value.length}`
            : "No matching repositories"}
        </span>
        <div className={cn("branch-results")}>
          {(() => {
            const entries = results.value;
            return entries.length === 0 ? (
              <>
                <p>{"No matching repositories."}</p>
              </>
            ) : (
              entries.map((repo, i) => (
                <Fragment key={repo.id}>
                  <div className={cn("quick-repo-row")}>
                    <Button
                      id={"repo-result-" + repo.id}
                      aria-current={index.value === i ? "true" : undefined}
                      onClick={() => onselect.value(repo)}
                      className={cn([
                        "commit-choice",
                        { "is-selected": index.value === i },
                      ])}
                    >
                      <span>
                        <strong>{repo.name}</strong>
                        <small>
                          {repo.pinned
                            ? "Pinned"
                            : recent.value.includes(repo)
                              ? "Recent"
                              : "Repository"}
                          {" · "}
                          {repo.branch || "Branch unavailable"}
                        </small>
                      </span>
                    </Button>
                    {repo.pinned && !query.value.trim() ? (
                      <>
                        <div className={cn("toolbar")}>
                          <Button
                            aria-label={"Move " + repo.name + " up"}
                            disabled={moving.value || i === 0}
                            onClick={() => move(repo, true)}
                            className={cn("text-action")}
                          >
                            {"↑"}
                          </Button>
                          <Button
                            aria-label={"Move " + repo.name + " down"}
                            disabled={
                              moving.value || i === pinned.value.length - 1
                            }
                            onClick={() => move(repo, false)}
                            className={cn("text-action")}
                          >
                            {"↓"}
                          </Button>
                        </div>
                      </>
                    ) : null}
                  </div>
                </Fragment>
              ))
            );
          })()}
          {matching.value.length > 60 ? (
            <>
              <p>{"Showing 60 results. Search to narrow the list."}</p>
            </>
          ) : null}
        </div>
        {selected.value && canOpen.value ? (
          <>
            <div className={cn("modal-actions")}>
              <Button
                variant={"secondary"}
                onClick={() => onlocation.value(selected.value, "editor")}
              >
                {"Open in editor"}
              </Button>
              <Button
                variant={"secondary"}
                onClick={() => onlocation.value(selected.value, "terminal")}
              >
                {"Open terminal"}
              </Button>
              <Button
                variant={"secondary"}
                onClick={() => onlocation.value(selected.value, "folder")}
              >
                {"Open folder"}
              </Button>
            </div>
          </>
        ) : null}
      </Modal>
    </>
  );
}
export default observer(function QuickSwitch(props: Props) {
  return useController(createView, props);
});
