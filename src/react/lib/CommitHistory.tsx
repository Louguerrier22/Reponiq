import {
FileCode,
GitCommitHorizontal,
LoaderCircle,
Search,
Undo2,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import { api,type Commit,type CommitDetails } from "../../lib/api";
import {
derived,
state,
untrack,
useController,
type Lifecycle
} from "../controller";
import { Button,Card,Input } from "../ui";
import { cn } from "../utils";
import DiffView from "./DiffView";
type Props = {
  repoId: number;
  head: string | null;
  blocked: boolean;
  onrevert: (hash: string, merge: boolean) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const repoId = derived(() => lifecycle.props().repoId);
  const head = derived(() => lifecycle.props().head);
  const blocked = derived(() => lifecycle.props().blocked);
  const onrevert = derived(() => lifecycle.props().onrevert);
  let commits = state<Commit[]>([]);
  let skip = state(0);
  let query = state("");
  let selected = state<Commit | null>(null);
  let detail = state<CommitDetails | null>(null);
  let file = state<string | null>(null);
  let fileQuery = state("");
  let code = state("");
  let busy = state(false);
  let diffBusy = state(false);
  let error = state("");
  let pageSequence = 0,
    selectionSequence = 0,
    diffSequence = 0,
    alive = true;
  const filtered = derived(() =>
    commits.value.filter((c) =>
      [c.subject, c.author, c.hash].some((text) =>
        text.toLocaleLowerCase().includes(query.value.toLocaleLowerCase()),
      ),
    ),
  );
  const files = derived(() =>
    (detail.value?.files ?? []).filter((f) =>
      f.path.toLocaleLowerCase().includes(fileQuery.value.toLocaleLowerCase()),
    ),
  );
  lifecycle.effect(() => {
    repoId.value;
    head.value;
    untrack(() => void loadPage(0));
  });
  lifecycle.destroy(() => {
    alive = false;
    ++pageSequence;
    ++selectionSequence;
    ++diffSequence;
  });
  async function loadPage(offset: number) {
    const sequence = ++pageSequence;
    const id = repoId.value;
    busy.value = true;
    error.value = "";
    selected.value = null;
    detail.value = null;
    code.value = "";
    ++selectionSequence;
    ++diffSequence;
    try {
      const rows = await api.history(id, offset);
      if (alive && sequence === pageSequence) {
        commits.value = rows;
        skip.value = offset;
        if (rows[0]) void choose(rows[0]);
      }
    } catch (e) {
      if (alive && sequence === pageSequence) error.value = String(e);
    } finally {
      if (alive && sequence === pageSequence) busy.value = false;
    }
  }
  async function choose(commit: Commit) {
    const sequence = ++selectionSequence;
    const id = repoId.value;
    selected.value = commit;
    detail.value = null;
    code.value = "";
    fileQuery.value = "";
    error.value = "";
    diffBusy.value = false;
    ++diffSequence;
    try {
      const result = await api.commitDetails(id, commit.hash);
      if (alive && sequence === selectionSequence) {
        detail.value = result;
        void showFile(result.files[0]?.path ?? null, commit.hash);
      }
    } catch (e) {
      if (alive && sequence === selectionSequence) error.value = String(e);
    }
  }
  async function showFile(path: string | null, hash = selected.value?.hash) {
    if (!hash) return;
    const sequence = ++diffSequence;
    const id = repoId.value;
    file.value = path;
    diffBusy.value = true;
    code.value = "";
    try {
      const result = await api.commitDiff(id, hash, path);
      if (alive && sequence === diffSequence) code.value = result;
    } catch (e) {
      if (alive && sequence === diffSequence) error.value = String(e);
    } finally {
      if (alive && sequence === diffSequence) diffBusy.value = false;
    }
  }
  return () => (
    <>
      <div className={cn("history-workbench")}>
        <Card
          as={"section"}
          aria-label={"Commit history"}
          className={cn("panel commit-browser")}
        >
          <div className={cn("panel-title")}>
            {"Commits "}
            <span>
              {skip.value + 1}
              {"–"}
              {skip.value + commits.value.length}
            </span>
          </div>
          <div className={cn("search-input history-search")}>
            <Search size={15} />
            <Input
              aria-label={"Find commits"}
              placeholder={"Find in this page"}
              value={query.value}
              onValueChange={(next) => {
                query.value = next;
              }}
            />
          </div>
          <div className={cn("commit-list")}>
            {(() => {
              const entries = filtered.value;
              return entries.length === 0 ? (
                <>
                  <p className={cn("empty-small")}>
                    {busy.value
                      ? "Loading commits…"
                      : query.value
                        ? "No matching commits"
                        : "No commits yet"}
                  </p>
                </>
              ) : (
                entries.map((commit, index) => (
                  <Fragment key={commit.hash}>
                    <Button
                      onClick={() => {
                        if (selected.value?.hash !== commit.hash)
                          void choose(commit);
                      }}
                      className={cn([
                        "commit-choice",
                        { "is-selected": selected.value?.hash === commit.hash },
                      ])}
                    >
                      <GitCommitHorizontal size={18} />
                      <span>
                        <strong>{commit.subject}</strong>
                        <small>
                          {commit.author}
                          {" · "}
                          {new Date(
                            commit.timestamp * 1000,
                          ).toLocaleDateString()}
                        </small>
                        <code>{commit.hash.slice(0, 8)}</code>
                      </span>
                    </Button>
                  </Fragment>
                ))
              );
            })()}
          </div>
          <div className={cn("history-paging")}>
            <Button
              variant={"secondary"}
              disabled={busy.value || skip.value === 0}
              onClick={() => loadPage(Math.max(0, skip.value - 50))}
            >
              {"Previous"}
            </Button>
            <span>{skip.value / 50 + 1}</span>
            <Button
              variant={"secondary"}
              disabled={busy.value || commits.value.length < 50}
              onClick={() => loadPage(skip.value + 50)}
            >
              {"Next"}
            </Button>
          </div>
        </Card>
        <section
          aria-label={"Selected commit"}
          className={cn("commit-inspector")}
        >
          {error.value ? (
            <>
              <p role={"alert"} className={cn("connection-error")}>
                {error.value}
              </p>
            </>
          ) : null}
          {selected.value && detail.value ? (
            <>
              <Card className={cn("panel commit-summary")}>
                <div>
                  <h2>{selected.value.subject}</h2>
                  <p>
                    {selected.value.author}
                    {" · "}
                    {new Date(selected.value.timestamp * 1000).toLocaleString()}
                  </p>
                  <code>{selected.value.hash}</code>
                </div>
                <Button
                  variant={"secondary"}
                  disabled={blocked.value}
                  title={
                    blocked.value
                      ? "Commit changes and finish any active operation first"
                      : "Create a new commit that undoes this commit"
                  }
                  onClick={() =>
                    onrevert.value(
                      selected.value!.hash,
                      detail.value!.parents.length > 1,
                    )
                  }
                >
                  <Undo2 size={15} />
                  {" Revert commit"}
                </Button>
                {detail.value.body.trim() !== selected.value.subject ? (
                  <>
                    <pre>{detail.value.body.slice(0, 20000)}</pre>
                  </>
                ) : null}
                {detail.value.parents.length > 1 ? (
                  <>
                    <small>
                      {
                        "Compared with the first parent. Reverting uses that same parent."
                      }
                    </small>
                  </>
                ) : null}
              </Card>
              <div className={cn("history-files-and-diff")}>
                <Card as={"section"} className={cn("panel history-files")}>
                  <div className={cn("panel-title")}>
                    {"Files "}
                    <span>
                      {detail.value.files.length}
                      {detail.value.truncated ? "+" : ""}
                    </span>
                  </div>
                  {detail.value.files.length > 8 ? (
                    <>
                      <Input
                        aria-label={"Filter commit files"}
                        placeholder={"Filter files"}
                        value={fileQuery.value}
                        onValueChange={(next) => {
                          fileQuery.value = next;
                        }}
                      />
                    </>
                  ) : null}
                  <div className={cn("file-list")}>
                    {(() => {
                      const entries = files.value.slice(0, 200);
                      return entries.map((item, index) => (
                        <Fragment key={item.path}>
                          <Button
                            onClick={() => {
                              if (file.value !== item.path)
                                void showFile(item.path);
                            }}
                            className={cn([
                              "changed-file",
                              { "file-selected": file.value === item.path },
                            ])}
                          >
                            <FileCode size={15} />
                            <span>{item.path}</span>
                            <b
                              className={cn(
                                item.status === "A" && "change-added",
                                item.status === "D" && "change-deleted",
                              )}
                            >
                              {item.status}
                            </b>
                          </Button>
                        </Fragment>
                      ));
                    })()}
                  </div>
                  {detail.value.truncated || files.value.length > 200 ? (
                    <>
                      <small className={cn("diff-limit")}>
                        {
                          "Preview limited. Filter files or use system Git for the full list."
                        }
                      </small>
                    </>
                  ) : null}
                </Card>
                <Card as={"section"} className={cn("panel diff-panel")}>
                  <div className={cn("panel-title")}>
                    <span className={cn("diff-filename")}>
                      {file.value ?? "All files"}
                    </span>
                    <Button
                      onClick={() => showFile(null)}
                      className={cn("text-action")}
                    >
                      {"All files"}
                    </Button>
                  </div>
                  {diffBusy.value ? (
                    <>
                      <div className={cn("diff-empty")}>
                        <LoaderCircle size={20} className={cn("spin")} />
                        {" Loading commit diff…"}
                      </div>
                    </>
                  ) : (
                    <>
                      {code.value ? (
                        <>
                          <DiffView code={code.value} label={"Commit diff"} />
                        </>
                      ) : (
                        <>
                          <div className={cn("diff-empty")}>
                            {"No textual changes"}
                          </div>
                        </>
                      )}
                    </>
                  )}
                </Card>
              </div>
            </>
          ) : (
            <>
              {selected.value ? (
                <>
                  <div role={"status"} className={cn("empty-block")}>
                    {"Loading commit…"}
                  </div>
                </>
              ) : null}
            </>
          )}
        </section>
      </div>
    </>
  );
}
export default observer(function CommitHistory(props: Props) {
  return useController(createView, props);
});
