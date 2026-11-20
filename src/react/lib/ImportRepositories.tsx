import {
ArrowLeft,
Check,
Code2,
Download,
FolderOpen,
LoaderCircle,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import {
api,
readablePath,
type Discovery,
type ImportSource,
} from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Checkbox,Label } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
import PlatformIcon from "./PlatformIcon";
type Props = {
  initial?: Discovery | null;
  defaultFolder?: string | null;
  onclose: () => void;
  onimported: (count: number) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const initial = derived(() => lifecycle.props().initial ?? null);
  const defaultFolder = derived(() => lifecycle.props().defaultFolder ?? null);
  const onclose = derived(() => lifecycle.props().onclose);
  const onimported = derived(() => lifecycle.props().onimported);
  let result = state<Discovery | null>(null),
    busy = state(false),
    loading = state(false),
    error = state(""),
    selected = state<string[]>([]),
    title = state("Import from"),
    sequence = 0,
    alive = true,
    addedTotal = 0;
  let failures = state<
    {
      path: string;
      message: string;
    }[]
  >([]);
  lifecycle.mount(() => {
    if (initial.value) {
      result.value = initial.value;
      selected.value = initial.value.items.map((r) => r.path);
      title.value = "Discovered repositories";
    }
  });
  lifecycle.destroy(() => {
    alive = false;
    ++sequence;
  });
  async function find(source: ImportSource) {
    const request = ++sequence;
    loading.value = true;
    error.value = "";
    result.value = null;
    title.value =
      source === "github_desktop"
        ? "GitHub Desktop"
        : source === "vs_code"
          ? "Visual Studio Code"
          : "Repository folder";
    try {
      let folder: string | null = null;
      if (source === "folder") {
        folder = await api.pickRepoFolder("parent");
        if (folder === null) {
          loading.value = false;
          title.value = "Import from";
          return;
        }
      }
      const value = await api.discover(source, folder);
      if (alive && request === sequence) {
        result.value = value;
        selected.value = value.items.map((r) => r.path);
      }
    } catch (e) {
      if (alive && request === sequence) error.value = String(e);
    } finally {
      if (alive && request === sequence) loading.value = false;
    }
  }
  async function importSelected() {
    if (busy.value || !selected.value.length) return;
    busy.value = true;
    error.value = "";
    try {
      const value = await api.importRepositories(selected.value);
      if (!alive) return;
      addedTotal += value.added;
      failures.value = value.failures ?? [];
      if (value.remaining.length) {
        selected.value = value.remaining;
        error.value = `Added ${value.added}. ${value.remaining.length} could not be added; review their folders and try again.`;
        if (result.value)
          result.value = {
            ...result.value,
            items: result.value.items.filter((r) =>
              value.remaining.includes(r.path),
            ),
          };
      } else onimported.value(addedTotal);
    } catch (e) {
      if (alive) error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  function toggle(path: string) {
    selected.value = selected.value.includes(path)
      ? selected.value.filter((p) => p !== path)
      : [...selected.value, path];
  }
  return () => (
    <>
      <Modal
        label={"Import repositories"}
        canClose={!busy.value && !loading.value}
        onclose={onclose.value}
      >
        <div className={cn("modal-top")}>
          <div>
            {result.value || error.value ? (
              <>
                <Button
                  disabled={busy.value || loading.value}
                  onClick={() => {
                    result.value = null;
                    error.value = "";
                    failures.value = [];
                    title.value = "Import from";
                  }}
                  className={cn("text-action add-back")}
                >
                  <ArrowLeft size={14} />
                  {" Back"}
                </Button>
              </>
            ) : null}
            <h2>{title.value}</h2>
          </div>
        </div>
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
        {(() => {
          const entries = failures.value;
          return entries.map((failure, index) => (
            <Fragment key={index}>
              <p className={cn("connection-error import-failure")}>
                <strong>{readablePath(failure.path)}</strong>
                <br />
                {failure.message}
              </p>
            </Fragment>
          ));
        })()}
        {loading.value ? (
          <>
            <p role={"status"} className={cn("import-loading")}>
              <LoaderCircle size={20} className={cn("spin")} />
              {" Finding repositories…"}
            </p>
          </>
        ) : (
          <>
            {result.value ? (
              <>
                {result.value.items.length ? (
                  <>
                    <Label className={cn("scope-choice")}>
                      <Checkbox
                        checked={
                          selected.value.length === result.value.items.length
                        }
                        disabled={busy.value}
                        onCheckedChange={() =>
                          (selected.value =
                            selected.value.length === result.value!.items.length
                              ? []
                              : result.value!.items.map((r) => r.path))
                        }
                      />
                      <span>
                        {"Select all · "}
                        {result.value.items.length}
                        {" repositories"}
                      </span>
                    </Label>
                    <div className={cn("import-results")}>
                      {(() => {
                        const entries = result.value.items;
                        return entries.map((repo, index) => (
                          <Fragment key={repo.path}>
                            <Label className={cn("import-repository")}>
                              <Checkbox
                                checked={selected.value.includes(repo.path)}
                                disabled={busy.value}
                                onCheckedChange={() => toggle(repo.path)}
                              />
                              <span>
                                <strong>{repo.name}</strong>
                                <small>{readablePath(repo.path)}</small>
                                <small>{repo.source}</small>
                              </span>
                            </Label>
                          </Fragment>
                        ));
                      })()}
                    </div>
                  </>
                ) : (
                  <>
                    <p className={cn("empty-small")}>
                      {
                        "No new repositories found. Already added repositories are not listed."
                      }
                    </p>
                  </>
                )}
                {(() => {
                  const entries = result.value.notes;
                  return entries.map((note, index) => (
                    <Fragment key={index}>
                      <p className={cn("folder-help")}>{note}</p>
                    </Fragment>
                  ));
                })()}
                {result.value.limited ? (
                  <>
                    <p className={cn("folder-help")}>
                      {
                        "Some folders were skipped or the scan limit was reached. Scan a smaller folder for more results."
                      }
                    </p>
                  </>
                ) : null}
                <div className={cn("modal-actions")}>
                  <Button
                    variant={"secondary"}
                    disabled={busy.value}
                    onClick={onclose.value}
                  >
                    {"Cancel"}
                  </Button>
                  <Button
                    variant={"primary"}
                    disabled={busy.value || !selected.value.length}
                    onClick={importSelected}
                  >
                    {busy.value ? (
                      <>
                        <LoaderCircle size={15} className={cn("spin")} />
                      </>
                    ) : (
                      <>
                        <Download size={15} />
                      </>
                    )}
                    {" Import "}
                    {selected.value.length}
                    {" repositories"}
                  </Button>
                </div>
              </>
            ) : (
              <>
                <div className={cn("add-choices")}>
                  <Button onClick={() => find("github_desktop")}>
                    <span>
                      <PlatformIcon remote={"https://github.com"} size={23} />
                    </span>
                    <div>
                      <strong>{"GitHub Desktop"}</strong>
                      <small>
                        {"Find recorded paths and scan your saved folder."}
                      </small>
                    </div>
                  </Button>
                  <Button onClick={() => find("vs_code")}>
                    <span>
                      <Code2 size={23} />
                    </span>
                    <div>
                      <strong>{"Visual Studio Code"}</strong>
                      <small>{"Find recent local Git folders."}</small>
                    </div>
                  </Button>
                  {defaultFolder.value ? (
                    <>
                      <Button onClick={() => find("default_folder")}>
                        <span>
                          <Check size={23} />
                        </span>
                        <div>
                          <strong>{"Default repository folder"}</strong>
                          <small>
                            {"Find repositories in your saved folder."}
                          </small>
                        </div>
                      </Button>
                    </>
                  ) : null}
                  <Button onClick={() => find("folder")}>
                    <span>
                      <FolderOpen size={23} />
                    </span>
                    <div>
                      <strong>{"Folder · any Git client"}</strong>
                      <small>
                        {"Choose a folder and review its repositories."}
                      </small>
                    </div>
                  </Button>
                </div>
              </>
            )}
          </>
        )}
      </Modal>
    </>
  );
}
export default observer(function ImportRepositories(props: Props) {
  return useController(createView, props);
});
