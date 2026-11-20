import { observer } from "mobx-react-lite";
import { api,type AppPreferences,type FetchSettings } from "../../lib/api";
import { confirm } from "../confirmation";
import {
SelectOption,
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Field,Label,NativeSelect,Switch } from "../ui";
import { cn } from "../utils";
type BaseProps = {
  initial?: boolean;
  busy?: boolean;
  ondone?: () => void | Promise<void>;
  ondrafts?: (prefs: AppPreferences) => void;
};
type Props = BaseProps & {
  onBusyChange?: (value: NonNullable<BaseProps["busy"]>) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const initial = derived(() => lifecycle.props().initial ?? false);
  const busy = lifecycle.bound("busy", false, "onBusyChange");
  const ondone = derived(() => lifecycle.props().ondone);
  const ondrafts = derived(() => lifecycle.props().ondrafts);
  const fetchScopeId = lifecycle.id;
  let fetchSettings = state<FetchSettings | null>(null);
  let autostart = state<boolean | null>(null);
  let startAtSignIn = state(false);
  let unavailable = state("");
  let error = state("");
  let saved = state(false);
  let alive = true;
  let workflow = state<AppPreferences | null>(null);
  let saveDrafts = state(true);
  function loadSettings() {
    error.value = "";
    void api
      .fetchSettings()
      .then((value) => {
        if (alive) fetchSettings.value = value;
      })
      .catch((reason) => {
        if (alive) error.value = String(reason);
      });
    if (initial.value)
      void api
        .workflowApp()
        .then((value) => {
          if (alive) {
            workflow.value = value;
            saveDrafts.value = value.save_drafts;
          }
        })
        .catch((reason) => {
          if (alive) error.value = String(reason);
        });
    void readAutostart();
  }
  lifecycle.mount(() => {
    loadSettings();
    return () => {
      alive = false;
    };
  });
  async function readAutostart() {
    try {
      const value = await api.autostartStatus();
      if (!alive) return;
      autostart.value = startAtSignIn.value = value;
      unavailable.value = "";
    } catch (reason) {
      if (alive) {
        autostart.value = null;
        unavailable.value = String(reason);
      }
    }
  }
  async function save() {
    if (!fetchSettings.value || busy.value) return;
    busy.value = true;
    error.value = "";
    saved.value = false;
    try {
      if (
        workflow.value?.save_drafts &&
        !saveDrafts.value &&
        !(await confirm({
          title: "Delete saved drafts on this device?",
          description: "Open drafts stay available until you close Reponiq.",
          action: "Delete saved drafts",
          destructive: true,
        }))
      )
        return;
      fetchSettings.value = await api.saveFetchSettings({
        ...fetchSettings.value,
      });
      if (
        autostart.value !== null &&
        (!startAtSignIn.value || startAtSignIn.value !== autostart.value)
      ) {
        const desired = startAtSignIn.value;
        autostart.value = startAtSignIn.value = await api.setAutostart(desired);
        if (autostart.value !== desired)
          throw new Error(
            "Your system did not apply the sign-in startup setting.",
          );
      }
      if (workflow.value && workflow.value.save_drafts !== saveDrafts.value) {
        workflow.value = await api.saveWorkflowApp({
          ...workflow.value,
          save_drafts: saveDrafts.value,
        });
        ondrafts.value?.(workflow.value);
      }
      saved.value = true;
      await ondone.value?.();
    } catch (reason) {
      error.value = String(reason);
      await readAutostart();
    } finally {
      busy.value = false;
    }
  }
  async function skip() {
    busy.value = true;
    error.value = "";
    try {
      await ondone.value?.();
    } catch (reason) {
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  return () => {
    const settings = fetchSettings.value;
    return (
      <>
        <div className={cn("startup-options")}>
          {settings ? (
            <>
              <Label className={cn("scope-choice")}>
                <Switch
                  checked={settings.enabled}
                  disabled={busy.value}
                  onCheckedChange={(next) => {
                    settings.enabled = next;
                    (() => (saved.value = false))();
                  }}
                />
                {" Fetch on launch"}
              </Label>
              <p className={cn("folder-help")}>
                {
                  "Refreshes remote branches in the background. You can start working immediately."
                }
              </p>
              <Field id={fetchScopeId} label={"Repositories to fetch"}>
                <NativeSelect
                  id={fetchScopeId}
                  value={settings.scope}
                  disabled={busy.value || !settings.enabled}
                  onValueChange={(next) => {
                    settings.scope = next;
                  }}
                  onChange={() => (saved.value = false)}
                >
                  <SelectOption value={"pinned"}>
                    {"Pinned repositories"}
                  </SelectOption>
                  <SelectOption value={"all"}>
                    {"All registered repositories"}
                  </SelectOption>
                </NativeSelect>
              </Field>
            </>
          ) : null}
          <Label className={cn("scope-choice")}>
            <Switch
              checked={startAtSignIn.value}
              disabled={busy.value || autostart.value === null}
              onCheckedChange={(next) => {
                startAtSignIn.value = next;
                (() => (saved.value = false))();
              }}
            />
            {" Start Reponiq at sign-in"}
          </Label>
          {initial.value ? (
            <>
              <Label className={cn("scope-choice")}>
                <Switch
                  checked={saveDrafts.value}
                  disabled={busy.value || !workflow.value}
                  onCheckedChange={(next) => {
                    saveDrafts.value = next;
                  }}
                />
                {" Save drafts on this device"}
              </Label>
            </>
          ) : null}
          {unavailable.value ? (
            <>
              <p className={cn("folder-help")}>{unavailable.value}</p>
            </>
          ) : null}
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
                {"Startup preferences saved."}
              </p>
            </>
          ) : null}
          <div className={cn("modal-actions")}>
            {error.value &&
            (!settings || (initial.value && !workflow.value)) ? (
              <>
                <Button
                  variant={"secondary"}
                  disabled={busy.value}
                  onClick={loadSettings}
                >
                  {"Retry loading preferences"}
                </Button>
              </>
            ) : null}
            {initial.value ? (
              <>
                <Button
                  disabled={busy.value}
                  onClick={skip}
                  className={cn("text-action")}
                >
                  {"Skip for now"}
                </Button>
              </>
            ) : null}
            <Button
              variant={"primary"}
              disabled={
                busy.value || !settings || (initial.value && !workflow.value)
              }
              onClick={save}
            >
              {initial.value ? "Save & continue" : "Save startup preferences"}
            </Button>
          </div>
        </div>
      </>
    );
  };
}
export default observer(function StartupSettings(props: Props) {
  return useController(createView, props);
});
