import {
  ArrowDownToLine,
  Check,
  ChevronLeft,
  ChevronRight,
  FileCode,
  GitBranch,
  Link,
  Search,
} from "lucide-react";
import {
  useId,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
} from "react";
import { api } from "../../lib/api";
import {
  defaultTheme,
  themeAttributes,
  themeVariables,
  type RegistryTheme,
  type Theme,
} from "../../lib/appearance";
import { Badge } from "../shadcn/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "../shadcn/dialog";
import { Button, Field, Input, ToggleGroup } from "../ui";
import { themeCatalog, type ThemeCatalogItem } from "./themeCatalog";
import "./theme-browser.css";

type Props = {
  theme: Theme;
  systemDark: boolean;
  busy?: boolean;
  onsave: (theme: Theme) => Promise<void>;
  oncontinue?: () => void;
};

const pageSize = 10;
const bundled = "reponiq";
const modes = [
  { value: "light", label: "Light" },
  { value: "dark", label: "Dark" },
  { value: "system", label: "System" },
];

function ThemePreview({
  theme,
  systemDark,
}: Pick<Props, "theme" | "systemDark">) {
  return (
    <section
      aria-label="Theme preview"
      className="theme-sample"
      {...themeAttributes(theme, systemDark)}
      style={themeVariables(theme, systemDark) as CSSProperties}
    >
      <div aria-hidden="true" className="theme-sample-header">
        <GitBranch size={16} /> <strong>main</strong>
        <span className="theme-sample-action">Fetch</span>
      </div>
      <div aria-hidden="true" className="theme-sample-workspace">
        <div className="theme-sample-nav">
          <b>Changes</b>
          <span>History</span>
          <span>Branches</span>
        </div>
        <div className="theme-sample-file">
          <span>
            <FileCode size={16} /> README.md <Check size={15} />
          </span>
          <code>
            <del>− Update notes</del>
            <ins>+ Update release notes</ins>
          </code>
        </div>
      </div>
    </section>
  );
}

function SourceTag({ external }: { external: boolean }) {
  return <Badge variant="outline">{external ? "External" : "In app"}</Badge>;
}

function themeName(theme: Theme) {
  return (
    theme.imported?.name ??
    (theme.preset === "reponiq" ? "Reponiq" : "Custom Reponiq")
  );
}

function validatePreview(theme: RegistryTheme) {
  for (const palette of [theme.light, theme.dark]) {
    if (
      Object.entries(palette).some(
        ([key, value]) =>
          !CSS.supports(key === "radius" ? "border-radius" : "color", value),
      )
    ) {
      throw new Error(
        "This theme uses colors or measurements your app cannot display.",
      );
    }
  }
  return theme;
}

function sameTheme(left: Theme, right: Theme) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function ThemeBrowser({
  theme,
  systemDark,
  onsave,
  onclose,
}: Pick<Props, "theme" | "systemDark" | "onsave"> & { onclose: () => void }) {
  const searchId = useId();
  const urlId = useId();
  const catalogScroll = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const [page, setPage] = useState(0);
  const [selection, setSelection] = useState<string | null>(
    sameTheme(theme, { ...defaultTheme, mode: theme.mode }) ? bundled : null,
  );
  const [mode, setMode] = useState(theme.mode);
  const [downloads, setDownloads] = useState<Record<string, RegistryTheme>>({});
  const [imported, setImported] = useState<{
    url: string;
    theme: RegistryTheme;
  }>();
  const [showImport, setShowImport] = useState(false);
  const [url, setUrl] = useState("");
  const [pending, setPending] = useState<string>();
  const [saving, setSaving] = useState(false);
  const [problem, setProblem] = useState<{
    selection: string | null;
    message: string;
  }>();

  const entries = [
    { name: "Reponiq", source: "In app", url: bundled },
    ...themeCatalog,
  ];
  const filtered = entries.filter((entry) =>
    [entry.name, entry.source, entry.url !== bundled ? "External" : "Reponiq"]
      .join(" ")
      .toLowerCase()
      .includes(query.trim().toLowerCase()),
  );
  const pageCount = Math.max(1, Math.ceil(filtered.length / pageSize));
  const visible = filtered.slice(page * pageSize, (page + 1) * pageSize);
  const selected = themeCatalog.find((entry) => entry.url === selection);
  const importedSelected = imported?.url === selection ? imported : undefined;
  const downloaded = selected
    ? downloads[selected.url]
    : importedSelected?.theme;
  const preview =
    selection === null
      ? { ...theme, mode }
      : selection === bundled
        ? { ...defaultTheme, mode }
        : downloaded && { ...defaultTheme, mode, imported: downloaded };
  const displayMode =
    mode === "system" ? (systemDark ? "dark" : "light") : mode;
  const name = selected?.name ?? themeName(preview ?? theme);
  const external = Boolean(selected || importedSelected || preview?.imported);
  const source = selected?.source ?? (external ? "Theme link" : "");
  const loadingSelected = pending === selection;
  const changed = preview && !sameTheme(preview, theme);

  function goToPage(next: number) {
    setPage(next);
    catalogScroll.current?.scrollTo({ top: 0 });
  }

  async function download(address: string, item?: ThemeCatalogItem) {
    if (pending || saving) return;
    setProblem(undefined);
    setPending(address);
    try {
      const result = validatePreview(await api.previewRegistryTheme(address));
      if (item) {
        setDownloads((previous) => ({
          ...previous,
          [item.url]: { ...result, name: item.name },
        }));
      } else {
        setImported({ url: address, theme: result });
        setSelection(address);
        setShowImport(false);
      }
    } catch (error) {
      setProblem({
        selection: item ? address : null,
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setPending(undefined);
    }
  }

  async function applyTheme() {
    if (!preview || saving || pending) return;
    setSaving(true);
    setProblem(undefined);
    try {
      await onsave(preview);
      onclose();
    } catch (error) {
      setProblem({
        selection,
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setSaving(false);
    }
  }

  function submitLink(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const address = url.trim();
    if (!address) return;
    const item = themeCatalog.find((entry) => entry.url === address);
    if (item) {
      setSelection(item.url);
      setShowImport(false);
      if (!downloads[item.url]) void download(item.url, item);
      return;
    }
    void download(address);
  }

  return (
    <DialogContent
      className="theme-browser"
      onEscapeKeyDown={(event) => {
        if (saving) event.preventDefault();
      }}
      onInteractOutside={(event) => {
        if (saving) event.preventDefault();
      }}
      showCloseButton={!saving}
    >
      <DialogHeader className="theme-browser-header">
        <DialogTitle>Change theme</DialogTitle>
        <DialogDescription>
          Browse and preview themes for Reponiq.
        </DialogDescription>
      </DialogHeader>
      <div className="theme-browser-toolbar">
        <div className="theme-browser-search">
          <Search aria-hidden="true" size={16} />
          <Input
            id={searchId}
            aria-label="Search themes"
            placeholder="Search themes"
            type="search"
            value={query}
            onValueChange={(value) => {
              setQuery(value);
              goToPage(0);
            }}
          />
        </div>
        <Button
          variant="secondary"
          aria-expanded={showImport}
          aria-controls={showImport ? urlId + "-form" : undefined}
          disabled={saving || Boolean(pending)}
          onClick={() => {
            setShowImport(!showImport);
            setProblem(undefined);
          }}
        >
          <Link size={15} /> Import link
        </Button>
      </div>
      {showImport && (
        <form
          id={urlId + "-form"}
          className="theme-link-form"
          onSubmit={submitLink}
        >
          <Field label="Theme JSON link" id={urlId}>
            <Input
              id={urlId}
              type="url"
              placeholder="https://…/theme.json"
              value={url}
              disabled={Boolean(pending)}
              onValueChange={(value) => {
                setUrl(value);
                setProblem(undefined);
              }}
            />
          </Field>
          <Button
            type="submit"
            variant="secondary"
            disabled={!url.trim() || Boolean(pending)}
          >
            {pending ? "Downloading…" : "Download theme"}
          </Button>
          {problem?.selection === null && (
            <p role="alert" className="connection-error">
              {problem.message}
            </p>
          )}
        </form>
      )}
      <div className="theme-browser-body">
        <section className="theme-browser-catalog" aria-label="Theme catalog">
          <div className="theme-browser-catalog-scroll" ref={catalogScroll}>
            {visible.length ? (
              <ul className="theme-browser-tiles">
                {visible.map((entry) => {
                  const isExternal = "swatches" in entry;
                  const swatches = isExternal
                    ? entry.swatches[displayMode]
                    : displayMode === "dark"
                      ? ["#24252b", "#292a31", "#c4b8fa"]
                      : ["#f2eef7", "#faf8ff", "#59448f"];
                  return (
                    <li key={entry.url}>
                      <Button
                        variant="ghost"
                        aria-pressed={selection === entry.url}
                        disabled={saving}
                        onClick={() => {
                          setSelection(entry.url);
                          setProblem(undefined);
                        }}
                      >
                        <span
                          className="theme-tile-swatches"
                          aria-hidden="true"
                        >
                          {swatches.map((color, index) => (
                            <i key={index} style={{ backgroundColor: color }} />
                          ))}
                        </span>
                        <span className="theme-tile-caption">
                          <strong>{entry.name}</strong>
                          <small>
                            {isExternal
                              ? "External · " + entry.source
                              : "In app"}
                          </small>
                        </span>
                      </Button>
                    </li>
                  );
                })}
              </ul>
            ) : (
              <p className="theme-empty">No themes match your search.</p>
            )}
          </div>
          <nav className="theme-browser-pagination" aria-label="Theme pages">
            <Button
              variant="ghost"
              aria-label="Previous page"
              disabled={page === 0}
              onClick={() => goToPage(page - 1)}
            >
              <ChevronLeft size={16} /> Previous
            </Button>
            <span aria-live="polite">
              {page + 1} / {pageCount}
            </span>
            <Button
              variant="ghost"
              aria-label="Next page"
              disabled={page + 1 >= pageCount}
              onClick={() => goToPage(page + 1)}
            >
              Next <ChevronRight size={16} />
            </Button>
          </nav>
        </section>
        <section className="theme-browser-preview" aria-label="Selected theme">
          <div className="theme-preview-title">
            <h3>{name}</h3>
            <SourceTag external={external} />
          </div>
          {source && <p className="theme-source">{source}</p>}
          <fieldset disabled={saving}>
            <legend className="sr-only">Preview mode</legend>
            <ToggleGroup
              label="Preview mode"
              options={modes}
              value={mode}
              onchange={(value) => setMode(value as Theme["mode"])}
            />
          </fieldset>
          {preview ? (
            <ThemePreview theme={preview} systemDark={systemDark} />
          ) : (
            selected && (
              <div
                className="theme-palette-preview"
                aria-label="Palette preview"
              >
                {selected.swatches[displayMode].map((color, index) => (
                  <i key={index} style={{ backgroundColor: color }} />
                ))}
                <p>
                  {loadingSelected
                    ? "Downloading theme…"
                    : "Download to preview in the app."}
                </p>
              </div>
            )
          )}
          {problem && problem.selection === selection && !showImport && (
            <p role="alert" className="connection-error">
              {problem.message}
            </p>
          )}
        </section>
      </div>
      <div className="theme-browser-footer">
        {selected && !downloaded ? (
          <Button
            variant="primary"
            disabled={Boolean(pending)}
            onClick={() => void download(selected.url, selected)}
          >
            <ArrowDownToLine size={16} />{" "}
            {loadingSelected ? "Downloading…" : "Download theme"}
          </Button>
        ) : (
          <Button
            variant="primary"
            disabled={!changed || saving || Boolean(pending)}
            onClick={() => void applyTheme()}
          >
            {saving ? "Applying…" : changed ? "Apply theme" : "Active theme"}
          </Button>
        )}
      </div>
    </DialogContent>
  );
}

export default function AppearanceEditor({
  theme,
  systemDark,
  busy,
  onsave,
  oncontinue,
}: Props) {
  const [open, setOpen] = useState(false);

  return (
    <div className="appearance-editor">
      <div className="theme-preview-title">
        <h3>{themeName(theme)}</h3>
        <Badge>Active</Badge>
        <SourceTag external={Boolean(theme.imported)} />
      </div>
      <ThemePreview theme={theme} systemDark={systemDark} />
      <div className="theme-settings-actions">
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogTrigger asChild>
            <Button variant="secondary" disabled={busy}>
              Change theme
            </Button>
          </DialogTrigger>
          {open && (
            <ThemeBrowser
              theme={theme}
              systemDark={systemDark}
              onsave={onsave}
              onclose={() => setOpen(false)}
            />
          )}
        </Dialog>
        {oncontinue && (
          <Button variant="primary" disabled={busy} onClick={oncontinue}>
            Continue
          </Button>
        )}
      </div>
    </div>
  );
}
