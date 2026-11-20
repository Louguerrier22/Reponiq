import { FolderOpen,LoaderCircle } from "lucide-react";
import { observer } from "mobx-react-lite";
import { api,readablePath,type WorkspaceSettings } from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Checkbox,Input,Label } from "../ui";
import { cn } from "../utils";
type BaseProps = {
  settings: WorkspaceSettings;
  initial?: boolean;
  completeOnSave?: boolean;
  busy?: boolean;
  dirty?: boolean;
  onsaved: (settings: WorkspaceSettings) => void;
};
type Props = BaseProps & {
  onBusyChange?: (value: NonNullable<BaseProps["busy"]>) => void;
  onDirtyChange?: (value: NonNullable<BaseProps["dirty"]>) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const settings = derived(() => lifecycle.props().settings);
  const initial = derived(() => lifecycle.props().initial ?? false);
  const completeOnSave = derived(
    () => lifecycle.props().completeOnSave ?? true,
  );
  const busy = lifecycle.bound("busy", false, "onBusyChange");
  const dirty = lifecycle.bound("dirty", false, "onDirtyChange");
  const onsaved = derived(() => lifecycle.props().onsaved);
  const folderId = lifecycle.id;
  let folder = state(""),
    detect = state(true),
    error = state(""),
    saved = state(false);
  lifecycle.effect(() => {
    folder.value = readablePath(settings.value.default_folder ?? "");
    detect.value = initial.value ? true : settings.value.auto_detect;
  });
  lifecycle.effect(() => {
    dirty.value =
      folder.value.trim() !==
        readablePath(settings.value.default_folder ?? "") ||
      (!!folder.value.trim() && detect.value !== settings.value.auto_detect);
  });
  async function browse() {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      const path = await api.pickRepoFolder("parent");
      if (path !== null) {
        folder.value = path;
        saved.value = false;
      }
    } catch (e) {
      error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  async function save(skip = false) {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      const value = await api.saveWorkspaceSettings({
        default_folder: skip ? null : folder.value.trim() || null,
        auto_detect: !skip && detect.value,
        setup_complete: completeOnSave.value,
      });
      saved.value = true;
      onsaved.value(value);
    } catch (e) {
      error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  return () => (
    <>
      <div className={cn("workspace-folder")}>
        <Label htmlFor={folderId}>{"Default repository folder"}</Label>
        <div className={cn("folder-picker")}>
          <Input
            id={folderId}
            placeholder={"Choose where your repositories live"}
            value={folder.value}
            disabled={busy.value}
            onValueChange={(next) => {
              folder.value = next;
            }}
            onInput={() => (saved.value = false)}
          />
          <Button variant={"secondary"} disabled={busy.value} onClick={browse}>
            <FolderOpen size={16} />
            {" Browse…"}
          </Button>
        </div>
        <Label className={cn("scope-choice")}>
          <Checkbox
            checked={detect.value}
            disabled={busy.value || !folder.value.trim()}
            onCheckedChange={(next) => {
              detect.value = next;
            }}
          />
          <span>{"Automatically find repositories in this folder"}</span>
        </Label>
        <p className={cn("folder-help")}>
          {"Review discovered repositories before adding them."}
        </p>
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
        {saved.value && !initial.value ? (
          <>
            <p role={"status"} className={cn("folder-saved")}>
              {"Folder saved"}
            </p>
          </>
        ) : null}
        <div className={cn("modal-actions")}>
          {initial.value ? (
            <>
              <Button
                disabled={busy.value}
                onClick={() => save(true)}
                className={cn("text-action")}
              >
                {"Skip for now"}
              </Button>
            </>
          ) : null}
          <Button
            variant={"primary"}
            disabled={busy.value || (initial.value && !folder.value.trim())}
            onClick={() => save()}
          >
            {busy.value ? (
              <>
                <LoaderCircle size={15} className={cn("spin")} />
              </>
            ) : null}
            {initial.value ? "Save & continue" : "Save folder"}
          </Button>
        </div>
      </div>
    </>
  );
}
export default observer(function WorkspaceFolder(props: Props) {
  return useController(createView, props);
});
