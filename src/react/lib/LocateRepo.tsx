import { observer } from "mobx-react-lite";
import { api,readablePath,type Location,type Repo } from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Checkbox,Input,Label } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
type Props = {
  repo: Repo;
  changeRoot?: boolean;
  mainWorktree?: string | null;
  onclose: () => void;
  onlocated: () => Promise<void>;
  onadded: (repo: Repo) => Promise<void>;
};
function createView(lifecycle: Lifecycle<Props>) {
  const repo = derived(() => lifecycle.props().repo);
  const mainWorktree = derived(() => lifecycle.props().mainWorktree);
  const onclose = derived(() => lifecycle.props().onclose);
  const onlocated = derived(() => lifecycle.props().onlocated);
  const onadded = derived(() => lifecycle.props().onadded);
  const changeRoot = derived(() => lifecycle.props().changeRoot);
  const folder = state(readablePath(repo.value.path));
  let preview = state<Location | null>(null),
    busy = state(false),
    acknowledged = state(false),
    error = state("");
  async function inspect(source: "browse" | "typed" | "main") {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    acknowledged.value = false;
    try {
      const path = source === "main" ? null
        : source === "browse" ? await api.pickRepoFolder() : folder.value.trim();
      if (source === "main" || path) {
        if (path) folder.value = readablePath(path);
        preview.value = await api.inspectLocation(repo.value, path);
      }
    } catch (reason) {
      preview.value = null;
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  async function apply() {
    if (busy.value || !preview.value) return;
    busy.value = true;
    error.value = "";
    try {
      if (preview.value.matching === "mismatch")
        await onadded.value(await api.add(preview.value.path));
      else {
        await api.relinkRepo(
          repo.value,
          preview.value.path,
          acknowledged.value,
        );
        await onlocated.value();
      }
    } catch (reason) {
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  return () => (
    <>
      <Modal
        label={changeRoot.value ? "Change root folder" : "Locate repository folder"}
        canClose={!busy.value}
        onclose={onclose.value}
      >
        <h2>{changeRoot.value ? "Change root folder" : `Locate ${repo.value.name}`}</h2>
        {changeRoot.value ? (
          <>
            <Label htmlFor="repository-root-folder">Repository folder</Label>
            <div className="folder-picker">
              <Input
                id="repository-root-folder"
                value={folder.value}
                disabled={busy.value}
                onValueChange={(next) => {
                  folder.value = next;
                  preview.value = null;
                  error.value = "";
                }}
              />
              <Button variant="secondary" disabled={busy.value} onClick={() => inspect("browse")}>
                Browse…
              </Button>
            </div>
          </>
        ) : <p>{readablePath(repo.value.path)}</p>}
        <div className={cn("toolbar")}>
          <Button
            variant="secondary"
            disabled={busy.value || (changeRoot.value && !folder.value.trim())}
            onClick={() => inspect(changeRoot.value ? "typed" : "browse")}
          >
            {changeRoot.value ? "Check folder" : "Choose Git folder…"}
          </Button>
          {mainWorktree.value ? (
            <>
              <Button
                variant={"secondary"}
                disabled={busy.value}
                onClick={() => inspect("main")}
              >
                {"Use main worktree"}
              </Button>
            </>
          ) : null}
        </div>
        {preview.value ? (
          <>
            <p>
              <strong>{readablePath(preview.value.path)}</strong>
            </p>
            <p>
              {"Branch "}
              {preview.value.branch}
              {" · "}
              {preview.value.remote ?? "Local repository"}
            </p>
            {preview.value.matching === "mismatch" ? (
              <>
                <p>
                  {
                    "This folder uses a different remote. You can add it as a separate repository."
                  }
                </p>
              </>
            ) : null}
            {preview.value.matching === "unknown" ? (
              <>
                <Label className={cn("scope-choice")}>
                  <Checkbox
                    checked={acknowledged.value}
                    disabled={busy.value}
                    onCheckedChange={(next) => {
                      acknowledged.value = next;
                    }}
                  />
                  {" I checked that this is the correct repository."}
                </Label>
              </>
            ) : null}
            <div className={cn("modal-actions")}>
              <Button
                variant={"primary"}
                disabled={
                  busy.value ||
                  (preview.value.matching === "unknown" && !acknowledged.value)
                }
                onClick={apply}
              >
                {preview.value.matching === "mismatch"
                  ? "Add as new"
                  : changeRoot.value ? "Change root folder" : "Relink repository"}
              </Button>
            </div>
          </>
        ) : null}
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
      </Modal>
    </>
  );
}
export default observer(function LocateRepo(props: Props) {
  return useController(createView, props);
});
