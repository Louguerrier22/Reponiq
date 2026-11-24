import { listen } from "@tauri-apps/api/event";
import { ArrowDownToLine,RotateCw } from "lucide-react";
import { observer } from "mobx-react-lite";
import { createContext,useContext,type ReactNode } from "react";
import { invoke } from "../../lib/diagnostics";
import { flushDraftWrites } from "../../lib/draft-store";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button, Label, ToggleGroup } from "../ui";
import { NativeSelect, NativeSelectOption } from "../shadcn/native-select";
import { version as installedVersion } from "../../../package.json";
import { cn } from "../utils";
import Modal from "./Modal";
const UpdatesContext = createContext({ ready: false, open: () => {} });

export function AppUpdateButton() {
  const { ready, open } = useContext(UpdatesContext);
  const label = ready ? "Update ready" : "App updates";
  return <Button data-tour="updates" title={label} onClick={open} className={cn("app-updates", ready && "update-ready")}><ArrowDownToLine size={17}/><span>{label}</span></Button>;
}

type BaseProps = {
  children: ReactNode;
  busy: boolean;
};
type Props = BaseProps;
type Selection = { beta: boolean; version: string | null };
function createView(lifecycle: Lifecycle<Props>) {
  const busy = derived(() => lifecycle.props().busy);
  type Status =
    | {
        state: "current" | "checking" | "unavailable";
      }
    | {
        state: "ready";
        version: string;
      };
  let status = state<Status>({ state: "checking" });
  let checking = state(false);
  let installing = state(false);
  let open = state(false);
  let error = state("");
  const selection = state<Selection>({ beta: false, version: null });
  const saved = state<Selection | null>(null);
  const versions = state<string[]>([]);
  const loadingVersions = state(false);
  const saving = state(false);
  let request = 0;
  async function loadVersions(beta: boolean) {
    const id = ++request;
    versions.value = [];
    loadingVersions.value = true;
    try {
      const result = await invoke<string[]>("app_update_versions", { beta });
      if (id === request) versions.value = result;
    } catch (reason) {
      if (id === request) error.value = String(reason);
    } finally {
      if (id === request) loadingVersions.value = false;
    }
  }
  async function showUpdates() {
    open.value = true;
    try {
      if (!saved.value) {
        const value = await invoke<Selection>("app_update_selection");
        saved.value = value;
        selection.value = value;
      }
      if (status.value.state !== "unavailable") await loadVersions(selection.value.beta);
    } catch (reason) {
      error.value = String(reason);
    }
  }
  const selectionChanged = derived(() => saved.value !== null && (
    selection.value.beta !== saved.value.beta || selection.value.version !== saved.value.version
  ));
  async function saveSelection() {
    if (saving.value || checking.value || installing.value || !selectionChanged.value) return;
    saving.value = true;
    error.value = "";
    try {
      const value = selection.value;
      await invoke("set_app_update_selection", { selection: value });
      saved.value = value;
      status.value = { state: "current" };
      await check();
    } catch (reason) {
      error.value = String(reason);
    } finally {
      saving.value = false;
    }
  }
  async function check() {
    if (checking.value || installing.value || status.value.state === "ready")
      return;
    checking.value = true;
    error.value = "";
    try {
      status.value = await invoke<Status>("check_app_update");
    } catch (reason) {
      error.value = String(reason);
    } finally {
      checking.value = false;
    }
  }
  async function restart() {
    if (busy.value || installing.value || status.value.state !== "ready")
      return;
    installing.value = true;
    error.value = "";
    try {
      await flushDraftWrites();
      await invoke("restart_for_update");
    } catch (reason) {
      error.value = String(reason);
      installing.value = false;
    }
  }
  lifecycle.mount(() => {
    let quitRequest: number | undefined;
    let frozenDialogs: HTMLDialogElement[] = [];
    function resumeAfterQuit(request: number) {
      if (quitRequest !== request) return;
      quitRequest = undefined;
      for (const dialog of frozenDialogs) dialog.inert = false;
      frozenDialogs = [];
      document.body.inert = false;
    }
    const subscription = listen<string>("app-update-error", (event) => {
      error.value = event.payload;
      installing.value = false;
      open.value = true;
      if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
    }).catch(() => undefined);
    const quitTimeout = listen<number>(
      "app-quit-timed-out",
      ({ payload: request }) => {
        if (quitRequest !== request) return;
        resumeAfterQuit(request);
        error.value =
          "Saving drafts took too long. Reponiq is still open. Try quitting again.";
        open.value = true;
      },
    ).catch(() => undefined);
    const quitSubscription = listen<number>(
      "app-quit-requested",
      async ({ payload: request }) => {
        if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
        quitRequest = request;
        document.body.inert = true;
        // Native modal dialogs escape inertness inherited from the document body.
        frozenDialogs = [
          ...document.querySelectorAll<HTMLDialogElement>(
            "dialog[open]:not([inert])",
          ),
        ];
        for (const dialog of frozenDialogs) dialog.inert = true;
        let draftsSaved = false;
        let accepted = false;
        try {
          try {
            await flushDraftWrites();
            draftsSaved = true;
          } catch (reason) {
            error.value = `Could not save the latest draft. ${String(reason)}`;
            open.value = true;
          }
          accepted = await invoke<boolean>("finish_app_quit", {
            request,
            draftsSaved,
          });
        } catch (reason) {
          error.value = String(reason);
          open.value = true;
        } finally {
          if (!accepted || !draftsSaved) resumeAfterQuit(request);
        }
      },
    )
      .then(async (unlisten) => {
        try {
          await invoke("register_app_quit_handler");
          return unlisten;
        } catch (reason) {
          unlisten();
          throw reason;
        }
      })
      .catch(() => undefined);
    const traySubscription = listen("open-app-updates", () => void showUpdates()).catch(() => undefined);
    void check();
    return () => {
      void traySubscription.then((unlisten) => unlisten?.());
      if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
      void subscription.then((unlisten) => unlisten?.());
      void quitTimeout.then((unlisten) => unlisten?.());
      void quitSubscription.then((unlisten) => unlisten?.());
    };
  });
  return () => (
    <>
      <UpdatesContext.Provider value={{ready:status.value.state==="ready",open:()=>{void showUpdates();}}}>
        {lifecycle.props().children}
      </UpdatesContext.Provider>
      {open.value ? (
        <>
          <Modal
            label={"App updates"}
            canClose={!installing.value}
            onclose={() => (open.value = false)}
          >
            <h2>App updates</h2>
            <p>Installed version {installedVersion}</p>
            {saved.value && status.value.state !== "unavailable" && (
              <div className="grid gap-2">
                <ToggleGroup
                  label="Update channel"
                  options={[{ value: "release", label: "Release" }, { value: "beta", label: "Beta" }]}
                  value={selection.value.beta ? "beta" : "release"}
                  disabled={checking.value || installing.value || saving.value}
                  onchange={(channel) => {
                    const beta = channel === "beta";
                    selection.value = { beta, version: null };
                    error.value = "";
                    void loadVersions(beta);
                  }}
                />
                <Label htmlFor="app-update-version">Version</Label>
                <NativeSelect
                  id="app-update-version"
                  value={selection.value.version ?? ""}
                  disabled={loadingVersions.value || checking.value || installing.value || saving.value}
                  onChange={(event) => (selection.value = { ...selection.value, version: event.target.value || null })}
                >
                  <NativeSelectOption value="">Latest in this channel</NativeSelectOption>
                  {selection.value.version && !versions.value.includes(selection.value.version) && (
                    <NativeSelectOption value={selection.value.version}>{selection.value.version}</NativeSelectOption>
                  )}
                  {versions.value.map((version) => (
                    <NativeSelectOption key={version} value={version}>{version}</NativeSelectOption>
                  ))}
                </NativeSelect>
                {loadingVersions.value && <p role="status">Loading versions…</p>}
              </div>
            )}
            {error.value ? (
              <>
                <p role={"alert"} className={cn("connection-error")}>
                  {error.value}
                </p>
              </>
            ) : null}
            {installing.value ? (
              <>
                <p role={"status"}>{"Installing the update…"}</p>
              </>
            ) : (
              <>
                {status.value.state === "ready" ? (
                  <>
                    <p>
                      {"Reponiq "}
                      {status.value.version}
                      {
                        " is ready. Restart now or fully quit Reponiq to apply the update."
                      }
                    </p>
                  </>
                ) : (
                  <>
                    {checking.value ? (
                      <>
                        <p role={"status"}>
                          {"Checking for and preparing updates…"}
                        </p>
                      </>
                    ) : (
                      <>
                        {status.value.state === "unavailable" ? (
                          <>
                            <p>
                              {
                                "This installation uses manual updates. Download the latest installer or update through your package manager."
                              }
                            </p>
                          </>
                        ) : (
                          <>
                            {!error.value ? (
                              <>
                                <p>{"No updates available."}</p>
                              </>
                            ) : null}
                          </>
                        )}
                      </>
                    )}
                  </>
                )}
              </>
            )}
            <div className={cn("modal-actions")}>
              {selectionChanged.value ? (
                <Button variant="primary" disabled={saving.value || checking.value || installing.value} onClick={saveSelection}>
                  Save and check
                </Button>
              ) : status.value.state === "ready" ? (
                <>
                  <Button
                    variant={"primary"}
                    disabled={busy.value || installing.value}
                    onClick={restart}
                  >
                    <RotateCw size={15} />
                    {" Restart and update"}
                  </Button>
                </>
              ) : (
                <>
                  {status.value.state !== "unavailable" ? (
                    <>
                      <Button
                        variant={"primary"}
                        disabled={checking.value}
                        onClick={check}
                      >
                        {"Check for updates"}
                      </Button>
                    </>
                  ) : null}
                </>
              )}
            </div>
          </Modal>
        </>
      ) : null}
    </>
  );
}
export default observer(function AppUpdates(props: Props) {
  return useController(createView, props);
});
