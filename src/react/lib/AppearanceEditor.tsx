import {
  Check,
  ChevronLeft,
  ChevronRight,
  FileCode,
  GitBranch,
  Link,
  Search,
} from "lucide-react";
import {
  useCallback,
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type FormEvent,
} from "react";
import { api } from "../../lib/api";
import {
  defaultTheme,
  registryVariables,
  themeAttributes,
  themeVariables,
  type RegistryTheme,
  type Theme,
} from "../../lib/appearance";
import { Badge } from "../shadcn/badge";
import { Button as SampleButton } from "../shadcn/button";
import { Card } from "../shadcn/card";
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
const sampleDefaults = Object.fromEntries(
  registryVariables
    .filter((key) => key.startsWith("--registry-"))
    .map((key) => [key, "initial"]),
);

function ThemePreview({
  theme,
  systemDark,
}: Pick<Props, "theme" | "systemDark">) {
  return (
    <section
      aria-label="Theme preview"
      className="theme-sample"
      {...themeAttributes(theme, systemDark)}
      style={{
        ...sampleDefaults,
        ...themeVariables(theme, systemDark),
      } as CSSProperties}
    >
      <div aria-hidden="true" className="theme-sample-header">
        <GitBranch size={16} /> <strong>main</strong>
        <div className="theme-sample-actions">
          <SampleButton asChild size="xs" variant="secondary">
            <span>Branches</span>
          </SampleButton>
          <SampleButton asChild size="xs">
            <span>Fetch</span>
          </SampleButton>
        </div>
      </div>
      <div aria-hidden="true" className="theme-sample-workspace">
        <div className="theme-sample-nav">
          <b>Changes</b>
          <span>History</span>
          <span>Stashes</span>
        </div>
        <Card className="theme-sample-file">
          <div className="theme-sample-file-header">
            <FileCode size={16} /> <strong>README.md</strong>
            <Badge variant="secondary">
              <Check size={12} /> Modified
            </Badge>
          </div>
          <code>
            <del>− Update notes</del>
            <ins>+ Update release notes</ins>
          </code>
        </Card>
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
  const [catalogPreviews, setCatalogPreviews] = useState<
    Record<string, { theme: RegistryTheme } | { error: string }>
  >({});
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
  const selectedResult = selected && catalogPreviews[selected.url];
  const externalPreview =
    selectedResult && "theme" in selectedResult
      ? selectedResult.theme
      : importedSelected?.theme;
  const preview =
    selection === null
      ? { ...theme, mode }
      : selection === bundled
        ? { ...defaultTheme, mode }
        : externalPreview && { ...defaultTheme, mode, imported: externalPreview };
  const displayMode =
    mode === "system" ? (systemDark ? "dark" : "light") : mode;
  const name = selected?.name ?? themeName(preview ?? theme);
  const external = Boolean(selected || importedSelected || preview?.imported);
  const source = selected?.source ?? (external ? "Theme link" : "");
  const changed = preview && !sameTheme(preview, theme);

  function goToPage(next: number) {
    setPage(next);
    catalogScroll.current?.scrollTo({ top: 0 });
  }

  const loadPreview = useCallback(async (address: string, item?: ThemeCatalogItem) => {
    setProblem(undefined);
    setPending(address);
    try {
      const result = validatePreview(await api.previewRegistryTheme(address));
      if (item) {
        setCatalogPreviews((previous) => ({
          ...previous,
          [item.url]: { theme: { ...result, name: item.name } },
        }));
      } else {
        setImported({ url: address, theme: result });
        setSelection(address);
        setShowImport(false);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (item) {
        setCatalogPreviews((previous) => ({
          ...previous,
          [item.url]: { error: message },
        }));
      } else {
        setProblem({ selection: null, message });
      }
    } finally {
      setPending(undefined);
    }
  }, []);

  useEffect(() => {
    if (selected && !selectedResult && !pending && !saving) {
      void loadPreview(selected.url, selected);
    }
  }, [selected, selectedResult, pending, saving, loadPreview]);

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
    if (!address || pending || saving) return;
    const item = themeCatalog.find((entry) => entry.url === address);
    if (item) {
      setSelection(item.url);
      setShowImport(false);
      return;
    }
    void loadPreview(address);
  }

  return (
    <DialogContent
      className="theme-browser"
      overlayClassName="theme-browser-overlay"
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
            {pending === url.trim() ? "Loading…" : "Preview theme"}
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
                        disabled={saving || (showImport && Boolean(pending))}
                        onClick={() => {
                          setSelection(entry.url);
                          setShowImport(false);
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
          ) : selected && selectedResult && "error" in selectedResult && pending !== selection ? (
            <div className="theme-preview-status">
              <p role="alert" className="connection-error">
                {selectedResult.error}
              </p>
              <Button
                variant="secondary"
                disabled={Boolean(pending)}
                onClick={() => void loadPreview(selected.url, selected)}
              >
                Retry preview
              </Button>
            </div>
          ) : (
            <p role="status" className="theme-preview-status">
              Loading preview…
            </p>
          )}
          {problem && problem.selection === selection && !showImport && (
            <p role="alert" className="connection-error">
              {problem.message}
            </p>
          )}
        </section>
      </div>
      <div className="theme-browser-footer">
        <Button
          variant="primary"
          disabled={!changed || saving || Boolean(pending)}
          onClick={() => void applyTheme()}
        >
          {saving ? "Applying…" : preview && !changed ? "Active theme" : "Apply theme"}
        </Button>
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
