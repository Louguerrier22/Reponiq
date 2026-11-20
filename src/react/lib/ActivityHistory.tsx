import {
Activity,
FolderGit2,
GitCommitHorizontal,
History,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import type { Activity as ActivityItem } from "../../lib/api";
import {
SelectOption,
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Badge,Label,NativeSelect } from "../ui";
import { cn } from "../utils";
type Props = {
  items: ActivityItem[];
  filterable?: boolean;
};
function createView(lifecycle: Lifecycle<Props>) {
  const items = derived(() => lifecycle.props().items);
  const filterable = derived(() => lifecycle.props().filterable ?? true);
  let filter = state("all");
  const day = new Intl.DateTimeFormat(undefined, { dateStyle: "full" });
  const time = new Intl.DateTimeFormat(undefined, { timeStyle: "short" });
  const kinds = {
    git: { label: "Git", icon: GitCommitHorizontal },
    ci: { label: "Workflow", icon: Activity },
    repo: { label: "Repository", icon: FolderGit2 },
  };
  const groups = derived(() => {
    const days = new Map<string, ActivityItem[]>();
    for (const item of items.value) {
      if (filter.value !== "all" && item.kind !== filter.value) continue;
      const key = day.format(item.occurred_at * 1000);
      const entries = days.get(key);
      if (entries) entries.push(item);
      else days.set(key, [item]);
    }
    return [...days];
  });
  function eventKind(kind: string) {
    return (
      kinds[kind as keyof typeof kinds] ?? { label: "Activity", icon: History }
    );
  }
  function eventTitle(item: ActivityItem) {
    if (item.kind !== "git" || !item.title.startsWith("Git: "))
      return item.title;
    const action = item.title.slice(5).replaceAll("_", " ");
    return action.charAt(0).toUpperCase() + action.slice(1);
  }
  return () => (
    <>
      <div className={cn("activity-feed")}>
        {filterable.value && items.value.length ? (
          <>
            <div className={cn("activity-filter")}>
              <Label htmlFor={"activity-type"}>{"Event type"}</Label>
              <NativeSelect
                id={"activity-type"}
                value={filter.value}
                onValueChange={(next) => {
                  filter.value = next;
                }}
              >
                <SelectOption value={"all"}>{"All events"}</SelectOption>
                {(() => {
                  const entries = Object.entries(kinds);
                  return entries.map(([key, kind], index) => (
                    <Fragment key={index}>
                      <SelectOption value={key}>{kind.label}</SelectOption>
                    </Fragment>
                  ));
                })()}
              </NativeSelect>
            </div>
          </>
        ) : null}
        {(() => {
          const entries = groups.value;
          return entries.length === 0 ? (
            <>
              <p className={cn("empty-small")}>
                {items.value.length
                  ? "No events of this type."
                  : "No activity yet."}
              </p>
            </>
          ) : (
            entries.map(([label, events], index) => (
              <Fragment key={label}>
                <section aria-label={label} className={cn("activity-day")}>
                  <h2>{label}</h2>
                  <ul>
                    {(() => {
                      const entries = events;
                      return entries.map((item, index) => (
                        <Fragment key={item.id}>
                          {(() => {
                            const kind = eventKind(item.kind);
                            return (
                              <>
                                <li>
                                  <span className={cn("activity-icon")}>
                                    <kind.icon size={16} aria-hidden={"true"} />
                                  </span>
                                  <div className={cn("activity-event")}>
                                    <strong>{eventTitle(item)}</strong>
                                    <span>
                                      {item.repo_name ??
                                        (item.kind === "ci"
                                          ? "GitHub"
                                          : "Workspace")}
                                    </span>
                                    {item.detail &&
                                    item.detail !== item.repo_name ? (
                                      <>
                                        <p>{item.detail}</p>
                                      </>
                                    ) : null}
                                  </div>
                                  <Badge className={cn("activity-kind")}>
                                    {kind.label}
                                  </Badge>
                                  <time
                                    dateTime={new Date(
                                      item.occurred_at * 1000,
                                    ).toISOString()}
                                  >
                                    {time.format(item.occurred_at * 1000)}
                                  </time>
                                </li>
                              </>
                            );
                          })()}
                        </Fragment>
                      ));
                    })()}
                  </ul>
                </section>
              </Fragment>
            ))
          );
        })()}
      </div>
    </>
  );
}
export default observer(function ActivityHistory(props: Props) {
  return useController(createView, props);
});
