import { Activity, ExternalLink, FolderGit2, GitCommitHorizontal, History } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, type Activity as ActivityItem } from "../../lib/api";
import { SelectOption } from "../controller";
import { Badge, Button, Label, NativeSelect } from "../ui";

const kinds = {
  git: { label: "Git", icon: GitCommitHorizontal },
  ci: { label: "Workflow run", icon: Activity },
  repo: { label: "Repository", icon: FolderGit2 },
};
const day = new Intl.DateTimeFormat(undefined, { dateStyle: "full" });
const time = new Intl.DateTimeFormat(undefined, { timeStyle: "short" });

export function activityTitle(item: ActivityItem) {
  if (item.kind !== "git" || !item.title.startsWith("Git: ")) return item.title;
  const action = item.title.slice(5).replaceAll("_", " ");
  return action.charAt(0).toUpperCase() + action.slice(1);
}

type Props = {
  items: ActivityItem[];
  selectedId: number | null;
  onSelect: (id: number | null) => void;
};

export default function ActivityHistory({ items, selectedId, onSelect }: Props) {
  const [filter, setFilter] = useState("all");
  const [error, setError] = useState<{ id: number; message: string } | null>(null);
  const selected = useRef<HTMLButtonElement>(null);
  const groups = new Map<string, ActivityItem[]>();

  for (const item of items) {
    if (filter !== "all" && item.kind !== filter) continue;
    const key = day.format(item.occurred_at * 1000);
    const events = groups.get(key);
    if (events) events.push(item);
    else groups.set(key, [item]);
  }

  useEffect(() => {
    selected.current?.scrollIntoView({ block: "nearest" });
  }, [selectedId]);

  async function openInBrowser(item: ActivityItem) {
    setError(null);
    try {
      await api.openActivityInBrowser(item.id);
    } catch (reason) {
      setError({ id: item.id, message: String(reason) });
    }
  }

  return (
    <div className="activity-feed">
      {items.length > 0 && (
        <div className="activity-filter">
          <Label htmlFor="activity-type">Event type</Label>
          <NativeSelect id="activity-type" value={filter} onValueChange={setFilter}>
            <SelectOption value="all">All events</SelectOption>
            {Object.entries(kinds).map(([key, kind]) => (
              <SelectOption key={key} value={key}>{kind.label}</SelectOption>
            ))}
          </NativeSelect>
        </div>
      )}
      {groups.size === 0 && (
        <p className="empty-small">{items.length ? "No events of this type." : "No activity yet."}</p>
      )}
      {[...groups].map(([label, events]) => (
        <section key={label} aria-label={label} className="activity-day">
          <h2>{label}</h2>
          <ul>
            {events.map((item) => {
              const { label, icon: Icon } = kinds[item.kind as keyof typeof kinds] ?? { label: "Activity", icon: History };
              const expanded = selectedId === item.id;
              const timestamp = new Date(item.occurred_at * 1000);
              const detail = item.kind === "ci" || item.detail === item.repo_name ? null : item.detail;
              return (
                <li key={item.id}>
                  <Button
                    ref={expanded ? selected : undefined}
                    className="activity-row"
                    aria-expanded={expanded}
                    onClick={() => onSelect(expanded ? null : item.id)}
                  >
                    <span className="activity-icon"><Icon size={16} aria-hidden="true" /></span>
                    <span className="activity-event">
                      <strong>{activityTitle(item)}</strong>
                      <span>{item.repo_name ?? (item.kind === "ci" ? "GitHub" : "Workspace")}</span>
                    </span>
                    <Badge className="activity-kind">{label}</Badge>
                    <time dateTime={timestamp.toISOString()}>{time.format(timestamp)}</time>
                  </Button>
                  {expanded && (
                    <div className="activity-details">
                      {detail && <p>{detail}</p>}
                      {!detail && <p><time dateTime={timestamp.toISOString()}>{timestamp.toLocaleString()}</time></p>}
                      {item.can_open_in_browser && (
                        <Button variant="secondary" onClick={() => void openInBrowser(item)}>
                          <ExternalLink size={15} />
                          Open in browser
                        </Button>
                      )}
                      {error?.id === item.id && <p role="alert">{error.message}</p>}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}
