import {
Copy,
Download,
LoaderCircle,
Pause,
Play,
Search,
Trash2,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import {
clearLogs,
exportLogs,
filterLogs,
formatLogs,
readLogs,
type LogEntry,
type LogLevel,
} from "../../lib/diagnostics";
import {
SelectOption,
derived,
state,
tick,
useController,
type Lifecycle
} from "../controller";
import { Button,Input,Label,NativeSelect } from "../ui";
import { cn } from "../utils";
import "./LiveLogs.css";
import Modal from "./Modal";
type Props = {
  onclose: () => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const onclose = derived(() => lifecycle.props().onclose);
  let entries = state<LogEntry[]>([]),
    level = state<LogLevel | "all">("all"),
    query = state("");
  let paused = state(false),
    follow = state(true),
    loading = state(true),
    exporting = state(false),
    clearing = state(false);
  let error = state(""),
    readError = state(""),
    notice = state(""),
    file = state<string | null>(null),
    persistenceError = state<string | null>(null),
    capacity = state(2000);
  let viewport: HTMLDivElement;
  let cursor = 0,
    fetching = false,
    alive = true,
    generation = 0;
  const visible = derived(() =>
    filterLogs(entries.value, level.value, query.value),
  );
  const errors = derived(
    () => entries.value.filter((e) => e.level === "error").length,
  );
  async function poll() {
    if (fetching || clearing.value) return;
    fetching = true;
    const request = generation;
    try {
      const result = await readLogs(cursor);
      if (!alive || request !== generation || paused.value) return;
      if (!result || !Array.isArray(result.entries))
        throw new Error("The logging service returned an unexpected response.");
      cursor = result.cursor;
      capacity.value = result.capacity;
      file.value = result.log_file;
      persistenceError.value = result.persistence_error;
      if (result.entries.length) {
        entries.value = [...entries.value, ...result.entries].slice(
          -capacity.value,
        );
        if (follow.value) {
          await tick();
          if (alive && viewport) viewport.scrollTop = viewport.scrollHeight;
        }
      }
      readError.value = "";
    } catch (e) {
      if (alive && request === generation) readError.value = String(e);
    } finally {
      fetching = false;
      if (alive) loading.value = false;
    }
  }
  lifecycle.mount(() => {
    void poll();
    const timer = setInterval(() => {
      if (!paused.value) void poll();
    }, 750);
    return () => {
      alive = false;
      generation++;
      clearInterval(timer);
    };
  });
  async function togglePause() {
    paused.value = !paused.value;
    if (!paused.value) await poll();
  }
  async function copy() {
    error.value = "";
    try {
      await navigator.clipboard.writeText(formatLogs(visible.value));
      notice.value = `Copied ${visible.value.length} log entries.`;
    } catch {
      error.value =
        "Could not copy logs. Use Download logs to save them instead.";
    }
  }
  async function download() {
    if (exporting.value) return;
    exporting.value = true;
    error.value = "";
    try {
      const path = await exportLogs(visible.value);
      if (alive && path) notice.value = `Saved logs to ${path}`;
    } catch (e) {
      if (alive) error.value = String(e);
    } finally {
      exporting.value = false;
    }
  }
  async function clear() {
    if (clearing.value) return;
    clearing.value = true;
    generation++;
    error.value = "";
    try {
      const result = await clearLogs();
      if (alive) {
        entries.value = [];
        cursor = result.cursor;
        persistenceError.value = result.persistence_error;
        notice.value = "Log history cleared.";
      }
    } catch (e) {
      if (alive) error.value = String(e);
    } finally {
      clearing.value = false;
    }
  }
  async function followLatest() {
    follow.value = !follow.value;
    if (follow.value) {
      await tick();
      if (viewport) viewport.scrollTop = viewport.scrollHeight;
    }
  }
  function time(timestamp: string) {
    const date = new Date(timestamp);
    return Number.isNaN(date.getTime())
      ? timestamp
      : date.toLocaleTimeString(undefined, {
          hour12: false,
          hour: "2-digit",
          minute: "2-digit",
          second: "2-digit",
        });
  }
  return () => (
    <>
      <Modal
        label={"Live logs"}
        canClose={!exporting.value}
        onclose={onclose.value}
      >
        <section className={cn("live-logs")}>
          <div className={cn("modal-top")}>
            <div>
              <h2>{"Live logs"}</h2>
              <p>{"Runtime activity and error details for troubleshooting."}</p>
            </div>
          </div>
          <div className={cn("log-summary")}>
            <span className={cn(paused.value && "paused")}>
              <i />
              {paused.value ? "Paused" : "Live"}
            </span>
            <span>
              {entries.value.length}
              {" retained · "}
              {errors.value} {errors.value === 1 ? "error" : "errors"}
            </span>
            <small>
              {"Latest "}
              {capacity.value.toLocaleString()}
              {" entries"}
            </small>
          </div>
          <div className={cn("log-toolbar")}>
            <div className={cn("search-input")}>
              <Search size={15} />
              <Input
                aria-label={"Search logs"}
                placeholder={"Search messages or sources"}
                value={query.value}
                onValueChange={(next) => {
                  query.value = next;
                }}
              />
            </div>
            <Label className={cn("log-level")}>
              {"Level"}
              <NativeSelect
                aria-label={"Log level"}
                value={level.value}
                onValueChange={(next) => {
                  level.value = next;
                }}
              >
                <SelectOption value={"all"}>{"All levels"}</SelectOption>
                <SelectOption value={"error"}>{"Errors"}</SelectOption>
                <SelectOption value={"warn"}>{"Warnings"}</SelectOption>
                <SelectOption value={"info"}>{"Info"}</SelectOption>
                <SelectOption value={"debug"}>{"Debug"}</SelectOption>
              </NativeSelect>
            </Label>
            <Button
              variant={"secondary"}
              aria-pressed={paused.value}
              onClick={togglePause}
            >
              {paused.value ? (
                <>
                  <Play size={14} />
                  {" Resume"}
                </>
              ) : (
                <>
                  <Pause size={14} />
                  {" Pause"}
                </>
              )}
            </Button>
          </div>
          {error.value ? (
            <>
              <p role={"alert"} className={cn("connection-error")}>
                {error.value}
              </p>
            </>
          ) : null}
          {readError.value ? (
            <>
              <p role={"alert"} className={cn("connection-error")}>
                {readError.value}{" "}
                <Button
                  onClick={() => {
                    paused.value = false;
                    void poll();
                  }}
                  className={cn("text-action")}
                >
                  {"Retry"}
                </Button>
              </p>
            </>
          ) : null}
          {persistenceError.value ? (
            <>
              <p role={"alert"} className={cn("connection-error")}>
                {persistenceError.value}
                {" Live entries are still available."}
              </p>
            </>
          ) : null}
          <div
            ref={(node) => {
              if (node) viewport = node;
            }}
            role={"region"}
            aria-label={"Log entries"}
            tabIndex={0}
            onScroll={() => {
              follow.value =
                viewport.scrollHeight -
                  viewport.scrollTop -
                  viewport.clientHeight <
                30;
            }}
            className={cn("log-viewport")}
          >
            {loading.value ? (
              <>
                <div role={"status"} className={cn("log-empty")}>
                  <LoaderCircle size={18} className={cn("spin")} />
                  {" Loading logs…"}
                </div>
              </>
            ) : (
              <>
                {!visible.value.length ? (
                  <>
                    <div className={cn("log-empty")}>
                      {entries.value.length
                        ? "No logs match these filters."
                        : "No log entries yet. Activity and errors will appear here."}
                    </div>
                  </>
                ) : (
                  <>
                    {(() => {
                      const entries = visible.value;
                      return entries.map((entry, index) => (
                        <Fragment key={entry.id}>
                          <article
                            className={cn(
                              "log-entry",
                              entry.level === "error" && "log-error",
                              entry.level === "warn" && "log-warning",
                            )}
                          >
                            <div className={cn("log-entry-meta")}>
                              <time
                                dateTime={entry.timestamp}
                                title={entry.timestamp}
                              >
                                {time(entry.timestamp)}
                              </time>
                              <span
                                className={cn("log-badge log-" + entry.level)}
                              >
                                {entry.level}
                              </span>
                              <span
                                title={entry.source}
                                className={cn("log-source")}
                              >
                                {entry.source}
                              </span>
                            </div>
                            <pre>{entry.message}</pre>
                          </article>
                        </Fragment>
                      ));
                    })()}
                  </>
                )}
              </>
            )}
          </div>
          <div className={cn("log-footer")}>
            <Button
              aria-pressed={follow.value}
              onClick={followLatest}
              className={cn("text-action")}
            >
              {follow.value ? "Following latest" : "Follow latest"}
            </Button>
            <span>
              {visible.value.length}
              {" shown"}
            </span>
            <div>
              <Button
                variant={"secondary"}
                disabled={!visible.value.length || clearing.value}
                onClick={copy}
              >
                <Copy size={14} />
                {" Copy logs"}
              </Button>
              <Button
                variant={"secondary"}
                disabled={
                  !visible.value.length || exporting.value || clearing.value
                }
                onClick={download}
              >
                {exporting.value ? (
                  <>
                    <LoaderCircle size={14} className={cn("spin")} />
                  </>
                ) : (
                  <>
                    <Download size={14} />
                  </>
                )}
                {" Download logs"}
              </Button>
              <Button
                variant={"secondary"}
                disabled={
                  clearing.value || exporting.value || !entries.value.length
                }
                onClick={clear}
              >
                <Trash2 size={14} />
                {" Clear logs"}
              </Button>
            </div>
          </div>
          {notice.value ? (
            <>
              <p role={"status"} className={cn("log-notice")}>
                {notice.value}
              </p>
            </>
          ) : null}
          <p className={cn("log-privacy")}>
            {
              "Logs stay on this device. Review local paths and private details before sharing."
            }
          </p>
          {file.value ? (
            <>
              <p className={cn("log-file")}>
                {"Saved locally: "}
                <code>{file.value}</code>
              </p>
            </>
          ) : null}
        </section>
      </Modal>
    </>
  );
}
export default observer(function LiveLogs(props: Props) {
  return useController(createView, props);
});
