import { observer } from "mobx-react-lite";
import { api,type ConflictDetails } from "../../lib/api";
import {
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Label,Textarea } from "../ui";
import { cn } from "../utils";
import Modal from "./Modal";
type Props = {
  repoId: number;
  file: string;
  onclose: () => void;
  onsaved: () => void;
  expected?: {
    branch: string;
    head: string | null;
  } | null;
};
function createView(lifecycle: Lifecycle<Props>) {
  const repoId = derived(() => lifecycle.props().repoId);
  const file = derived(() => lifecycle.props().file);
  const onclose = derived(() => lifecycle.props().onclose);
  const onsaved = derived(() => lifecycle.props().onsaved);
  const expected = derived(() => lifecycle.props().expected ?? null);
  let data = state<ConflictDetails | null>(null);
  let content = state("");
  let error = state("");
  let busy = state(false);
  let deleteConfirm = state(false);
  let alive = true;
  lifecycle.mount(() => {
    void api
      .conflict(repoId.value, file.value)
      .then((value) => {
        if (alive) {
          data.value = value;
          content.value = value.current;
        }
      })
      .catch((e) => {
        if (alive) error.value = String(e);
      });
    return () => {
      alive = false;
    };
  });
  async function save(remove = false) {
    if (!data.value || busy.value) return;
    if (remove && !deleteConfirm.value) {
      deleteConfirm.value = true;
      return;
    }
    busy.value = true;
    error.value = "";
    try {
      await api.git(
        repoId.value,
        remove ? "resolve_delete" : "resolve_file",
        remove
          ? [file.value, data.value.current]
          : [file.value, content.value, data.value.current],
        expected.value,
      );
      onsaved.value();
      onclose.value();
    } catch (e) {
      error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  return () => (
    <>
      <Modal
        canClose={!busy.value}
        label={"Resolve conflict"}
        onclose={onclose.value}
      >
        <div className={cn("modal-top")}>
          <div>
            <h2>{"Resolve conflict"}</h2>
            <p className={cn("conflict-path")}>{file.value}</p>
          </div>
        </div>
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
        {data.value ? (
          <>
            <div className={cn("conflict-sides")}>
              <section>
                <h3>{"Current side"}</h3>
                <pre>{data.value.ours ?? "File deleted on this side"}</pre>
                <Button
                  variant={"secondary"}
                  disabled={busy.value || data.value.ours === null}
                  onClick={() => (content.value = data.value!.ours!)}
                >
                  {"Use current"}
                </Button>
              </section>
              <section>
                <h3>{"Incoming side"}</h3>
                <pre>{data.value.theirs ?? "File deleted on this side"}</pre>
                <Button
                  variant={"secondary"}
                  disabled={busy.value || data.value.theirs === null}
                  onClick={() => (content.value = data.value!.theirs!)}
                >
                  {"Use incoming"}
                </Button>
              </section>
            </div>
            <Label htmlFor={"resolved-content"}>{"Resolved contents"}</Label>
            <Textarea
              id={"resolved-content"}
              rows={12}
              value={content.value}
              disabled={busy.value}
              spellCheck={"false"}
              onValueChange={(next) => {
                content.value = next;
              }}
              className={cn("conflict-result")}
            />
            {deleteConfirm.value ? (
              <>
                <p className={cn("connection-error")}>
                  {
                    "This will delete the working file and stage that deletion as the conflict resolution."
                  }
                </p>
              </>
            ) : null}
            <div className={cn("modal-actions")}>
              {data.value.ours === null || data.value.theirs === null ? (
                <>
                  <Button
                    disabled={busy.value}
                    onClick={() => save(true)}
                    className={cn("dangerous secondary")}
                  >
                    {deleteConfirm.value
                      ? "Confirm deletion & stage"
                      : "Keep deletion"}
                  </Button>
                </>
              ) : null}
              <Button
                variant={"secondary"}
                disabled={busy.value}
                onClick={onclose.value}
              >
                {"Cancel"}
              </Button>
              <Button
                variant={"primary"}
                disabled={busy.value}
                onClick={() => save()}
              >
                {"Save & stage"}
              </Button>
            </div>
          </>
        ) : (
          <>
            {!error.value ? (
              <>
                <p role={"status"}>{"Loading conflict…"}</p>
              </>
            ) : (
              <>
                <div className={cn("modal-actions")}>
                  <Button variant={"secondary"} onClick={onclose.value}>
                    {"Close"}
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
export default observer(function ConflictEditor(props: Props) {
  return useController(createView, props);
});
