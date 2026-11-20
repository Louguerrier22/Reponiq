import { observer } from "mobx-react-lite";
import { api,readablePath,type Location,type Repo } from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Checkbox,Label } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
type Props = {
  repo: Repo;
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
  let preview = state<Location | null>(null),
    busy = state(false),
    acknowledged = state(false),
    error = state("");
  async function inspect(main = false) {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    acknowledged.value = false;
    try {
      const path = main ? null : await api.pickRepoFolder();
      if (main || path)
        preview.value = await api.inspectLocation(repo.value, path);
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
        label={"Locate repository folder"}
        canClose={!busy.value}
        onclose={onclose.value}
      >
        <h2>
          {"Locate "}
          {repo.value.name}
        </h2>
        <p>{readablePath(repo.value.path)}</p>
        <div className={cn("toolbar")}>
          <Button
            variant={"secondary"}
            disabled={busy.value}
            onClick={() => inspect()}
          >
            {"Choose Git folder…"}
          </Button>
          {mainWorktree.value ? (
            <>
              <Button
                variant={"secondary"}
                disabled={busy.value}
                onClick={() => inspect(true)}
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
                  : "Relink repository"}
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
