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
import { Button } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
const UpdatesContext = createContext({ ready: false, open: () => {} });

export function AppUpdateButton() {
  const { ready, open } = useContext(UpdatesContext);
  const label = ready ? "Update ready" : "App updates";
  return <Button title={label} onClick={open} className={cn("app-updates", ready && "update-ready")}><ArrowDownToLine size={17}/><span>{label}</span></Button>;
}

type BaseProps = {
  children: ReactNode;
  busy: boolean;
};
type Props = BaseProps & {};
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
    void check();
    return () => {
      if (quitRequest !== undefined) resumeAfterQuit(quitRequest);
      void subscription.then((unlisten) => unlisten?.());
      void quitTimeout.then((unlisten) => unlisten?.());
      void quitSubscription.then((unlisten) => unlisten?.());
    };
  });
  return () => (
    <>
      <UpdatesContext.Provider value={{ready:status.value.state==="ready",open:()=>{open.value=true;}}}>
        {lifecycle.props().children}
      </UpdatesContext.Provider>
      {open.value ? (
        <>
          <Modal
            label={"App updates"}
            canClose={!installing.value}
            onclose={() => (open.value = false)}
          >
            <h2>{"App updates"}</h2>
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
              <Button
                variant={"secondary"}
                disabled={installing.value}
                onClick={() => (open.value = false)}
              >
                {"Close"}
              </Button>
              {status.value.state === "ready" ? (
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
