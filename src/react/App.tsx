import {
  Activity,
  ArrowDown,
  ArrowLeftRight,
  ArrowUp,
  Check,
  CircleAlert,
  Clock3,
  Code2,
  FileCode,
  Folder,
  FolderGit2,
  GitBranch,
  GitCommitHorizontal,
  History,
  LayoutDashboard,
  LayoutGrid,
  List,
  LoaderCircle,
  Pin,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Tags,
  X,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment, Suspense, lazy } from "react";
import { toast } from "sonner";
import { version as appVersion } from "../../package.json";
import {
  GitFailure,
  api,
  filteredRepos,
  readablePath,
  type Account,
  type Activity as ActivityItem,
  type AppPreferences,
  type Discovery,
  type FetchSettings,
  type GitExpectation,
  type LoginCode,
  type Repo,
  type RepoContext,
  type RepoDetails,
  type RepoPreferences,
  type RepoSummary,
  type RepoWorkflow,
  type WorkspaceSettings,
} from "../lib/api";
import {
  defaultTheme,
  registryVariables,
  themeAttributes,
  themeVariables,
  type SetupStep,
  type Theme,
} from "../lib/appearance";
import {
  draftKey,
  forgetRepositoryDrafts,
  readDraft,
  saveDraft,
  type DraftContext,
} from "../lib/draft-store";
import { launchFetch, repoFetchEnabled } from "../lib/launch-fetch";
import { remoteInfo } from "../lib/platform";
import ConfirmDialog from "./ConfirmDialog";
import { confirm } from "./confirmation";
import {
  LoadBoundary,
  WindowEvents,
  derived,
  state,
  tick,
  untrack,
  useController,
  type Lifecycle,
} from "./controller";
import { Github } from "./icons";
import ActivityHistory, { activityTitle } from "./lib/ActivityHistory";
import { CollectionActions, collectionDragStart, collectionDropTarget } from "./lib/CollectionActions";
import CollectionMenuItem from "./lib/CollectionMenuItem";
import AccountsSettings from "./lib/AccountsSettings";
import AppUpdates, { AppUpdateButton } from "./lib/AppUpdates";
import DiffView from "./lib/DiffView";
import LocateRepo from "./lib/LocateRepo";
import Modal from "./lib/Modal";
import PlatformIcon from "./lib/PlatformIcon";
import QuickSwitch from "./lib/QuickSwitch";
import TokenInput from "./lib/TokenInput";
import Walkthrough from "./lib/Walkthrough";
import WorkspaceFolder from "./lib/WorkspaceFolder";
import {
  SidebarFooter,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
} from "./shadcn/sidebar";
import {
  Alert,
  Badge,
  Button,
  Card,
  Checkbox,
  Collapsible,
  Empty,
  Input,
  Label,
  Progress,
  Sidebar,
  SidebarMenuButton,
  Switch,
  Tabs,
  TabsList,
  TabsTrigger,
  Textarea,
  Toaster,
  ToggleGroup,
} from "./ui";
import { Button as ShadcnButton } from "./shadcn/button";
import { Popover, PopoverClose, PopoverContent, PopoverTrigger } from "./shadcn/popover";
import { Field as ShadcnField, FieldLabel } from "./shadcn/field";
import { cn } from "./utils";
const PullRequests = lazy(() => import("./lib/PullRequests"));
const CommitHistory = lazy(() => import("./lib/CommitHistory"));
const Branches = lazy(() => import("./lib/Branches"));
const AddRepository = lazy(() => import("./lib/AddRepository"));
const ImportRepositories = lazy(() => import("./lib/ImportRepositories"));
const LiveLogs = lazy(() => import("./lib/LiveLogs"));
const ConflictEditor = lazy(() => import("./lib/ConflictEditor"));
const WorkflowSettings = lazy(() => import("./lib/WorkflowSettings"));
const StartupSettings = lazy(() => import("./lib/StartupSettings"));
const AppearanceEditor = lazy(() => import("./lib/AppearanceEditor"));
const SetupIdentity = lazy(() => import("./lib/SetupIdentity"));
type BaseProps = {};
type Props = BaseProps & {};
function createView(lifecycle: Lifecycle<Props>) {
  let repos = state<Repo[]>([]);
  let activities = state<ActivityItem[]>([]);
  const activeActivityId = state<number | null>(null);
  let accounts = state<Account[]>([]);
  let view = state("all");
  const overviewChoice = state<"all" | "collections" | null>(null);
  const settingsTab = state<"general" | "accounts">("general");
  let query = state("");
  let selected = state<number | null>(null);
  let details = state<RepoDetails | null>(null);
  let working = state(false);
  let loading = state(true);
  let shelfLayout = state<"grid" | "rows">("grid");
  const shelfModes = [
    { value: "grid", label: "Grid view" },
    { value: "rows", label: "Row view" },
  ];
  const commitModes = [
    { value: "exclude", label: "Exclude checked" },
    { value: "include", label: "Include checked" },
  ];
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  function showNotice(message: string) {
    clearTimeout(noticeTimer);
    toast.success(message, { id: "reponiq-feedback" });
    noticeTimer = setTimeout(() => toast.dismiss("reponiq-feedback"), 5000);
  }
  let error = state("");
  let showAdd = state(false);
  let showBranches = state(false);
  let showSavedChanges = state(false);
  let startupBusy = state(false);
  let identityBusy = state(false);
  let setupFolderDirty = state(false);
  let advancingSetup = state(false);
  let setupStep = state<SetupStep>("complete");
  let showGuideChoice = state(false);
  let guideBusy = state(false);
  let setupError = state("");
  let guideError = state("");
  let theme = state<Theme>(defaultTheme);
  let systemDark = state(true);
  let appearanceReady = state(false);
  let appearanceError = state("");
  const resolvedTheme = derived(() =>
    themeAttributes(theme.value, systemDark.value),
  );
  const setupSteps = ["appearance", "workspace", "accounts", "startup", "review"] as const;
  const repositoryActivity = derived(() => activities.value.filter((item) => item.repo_id === selected.value));
  let launchFailures = state<Record<number, string>>({}),
    retryingLaunchFetch = state(false);
  let launchBatch = Date.now();
  const launchPreferences = state<FetchSettings | null>(null);
  const savingRepoFetch = state(new Set<number>());
  let metadata = state(new Map<number, RepoWorkflow>());
  let repoMetadataReady = state(false),
    workflowReady = state(false);
  let workflowApp = state<AppPreferences>({
    save_drafts: true,
    warn_default: true,
    editor: null,
  });
  let showQuick = state(false),
    showRepoSettings = state(false),
    locating = state<Repo | null>(null);
  const changingRoot = state(false);
  const renaming = state<{ repo: Repo; name: string } | null>(null);
  const removing = state<{ repo: Repo; deleteFolder: boolean } | null>(null);
  const repoDialogError = state("");
  let repoContext = state<RepoContext | null>(null),
    recentBranches = state<string[]>([]);
  let defaultCommit = state<{
    repo: Repo;
    branch: string;
    head: string | null;
    message: string;
    files: string[];
  } | null>(null);
  let selectionMode = state<"include" | "exclude">("exclude");
  let markedFiles = state(new Set<string>());
  let branchNameForCommit = state(""),
    undoConfirm = state<{
      repo: Repo;
      branch: string;
      head: string;
    } | null>(null),
    recoveredMessage = state("");
  let showFileSearch = state(false),
    showFolded = state(false);
  let fileSearch = state<HTMLInputElement>();
  let gitFailure = state<{
    code: string;
    branch: string | null;
  } | null>(null);
  let showWalkthrough = state(false);
  let showLiveLogs = state(false);
  let workspace = state<WorkspaceSettings>({
    default_folder: null,
    auto_detect: false,
    setup_complete: false,
  });
  let showSetup = state(false),
    setupBusy = state(false),
    showImport = state(false);
  let found = state<Discovery | null>(null),
    importInitial = state<Discovery | null>(null);
  let discoverySequence = 0;
  let detectionBusy = false;
  let discoveryError = state("");
  async function scanDefault() {
    if (
      detectionBusy ||
      !workspace.value.auto_detect ||
      !workspace.value.default_folder
    )
      return;
    detectionBusy = true;
    const sequence = ++discoverySequence;
    try {
      const result = await api.discover("default_folder");
      if (sequence === discoverySequence) {
        found.value = result;
        discoveryError.value = "";
      }
    } catch (e) {
      if (sequence === discoverySequence) discoveryError.value = String(e);
    } finally {
      detectionBusy = false;
    }
  }
  async function loadAppearance() {
    appearanceError.value = "";
    try {
      const settings = await api.appearance();
      if (!alive) return;
      theme.value = settings.theme;
      setupStep.value = settings.setup_step;
      appearanceReady.value = true;
      showSetup.value = setupStep.value !== "complete";
      if (!showSetup.value) void fetchOnLaunch();
    } catch (reason) {
      if (alive) appearanceError.value = String(reason);
    }
  }
  lifecycle.effect(() => {
    for (const [key, value] of Object.entries(resolvedTheme.value)) {
      if (value === undefined) document.documentElement.removeAttribute(key);
      else document.documentElement.setAttribute(key, value);
    }
    const tokens = themeVariables(theme.value, systemDark.value);
    for (const variable of registryVariables) {
      if (tokens[variable])
        document.documentElement.style.setProperty(variable, tokens[variable]);
      else document.documentElement.style.removeProperty(variable);
    }
  });
  async function saveAppearance(next: Theme) {
    theme.value = await api.saveAppearance(next);
  }
  async function continueSetup(expected: SetupStep) {
    if (advancingSetup.value) return;
    advancingSetup.value = true;
    setupError.value = "";
    try {
      setupStep.value = await api.advanceSetup(expected);
      if (setupStep.value === "complete") {
        workspace.value = { ...workspace.value, setup_complete: true };
        showSetup.value = false;
        showGuideChoice.value = true;
        void fetchOnLaunch();
        selected.value = null;
        view.value = "all";
      }
    } catch (reason) {
      setupError.value = String(reason);
      throw reason;
    } finally {
      advancingSetup.value = false;
    }
  }
  function updateWorkspace(settings: WorkspaceSettings) {
    workspace.value = settings;
    ++discoverySequence;
    found.value = null;
    discoveryError.value = "";
    void scanDefault();
  }
  async function chooseGuide(start: boolean) {
    if (guideBusy.value) return;
    guideBusy.value = true;
    guideError.value = "";
    try {
      if (!start) await api.finishWalkthrough();
      showGuideChoice.value = false;
      if (start) showWalkthrough.value = true;
    } catch (reason) {
      guideError.value = String(reason);
    } finally {
      guideBusy.value = false;
    }
  }
  function openImport(initial: Discovery | null = null) {
    importInitial.value = initial;
    showImport.value = true;
  }
  async function imported(count: number) {
    showImport.value = false;
    found.value = null;
    query.value = "";
    view.value = "all";
    selected.value = null;
    showNotice(`Imported ${count} repositories`);
    await load();
  }
  let showAccount = state(false);
  let showOrganize = state(false);
  let organizing = state(false);
  let pendingPins = state<number[]>([]);
  let addingCollection = state(false);
  let organizationId = state<number | null>(null);
  let collectionName = state("");
  const existingCollection = state<string | null>(null);
  const collectionRepos = state(new Set<number>());
  const collectionQuery = state("");
  let organizationError = state("");
  const organizationRepo = derived(() =>
    repos.value.find((repo) => repo.id === organizationId.value),
  );
  let alive = true,
    metadataRevision = 0;
  const localRevisions = new Map<number, number>();
  const statusVersions = new Map<number, number>();
  const statusQueue = new Set<number>();
  const activeStatuses = new Set<number>();
  function changedMetadata(id: number) {
    localRevisions.set(id, ++metadataRevision);
  }
  function requestStatuses(ids: number[]) {
    for (const id of ids) if (!activeStatuses.has(id)) statusQueue.add(id);
    drainStatuses();
  }
  function applySummary(summary: RepoSummary) {
    repos.value = repos.value.map((repo) => {
      if (
        repo.id !== summary.id ||
        repo.path !== summary.path ||
        repo.registration !== summary.registration
      )
        return repo;
      const counts = summary.status_error
        ? !repo.branch
          ? (workflowFor(repo)?.cached_status ?? {})
          : {}
        : {
            branch: summary.branch,
            dirty: summary.dirty,
            ahead: summary.ahead,
            behind: summary.behind,
            conflicts: summary.conflicts,
          };
      return {
        ...repo,
        ...counts,
        missing: summary.missing,
        status_error: summary.status_error,
        status_pending: false,
      };
    });
  }
  function drainStatuses() {
    if (!alive) return;
    while (activeStatuses.size < 2) {
      const id = [...statusQueue].find(
        (id) => !(working.value && id === selected.value),
      );
      if (id === undefined) return;
      statusQueue.delete(id);
      if (!repos.value.some((r) => r.id === id)) continue;
      activeStatuses.add(id);
      const version = statusVersions.get(id) ?? 0;
      void api
        .summary(id)
        .then((summary) => {
          if (alive && (statusVersions.get(id) ?? 0) === version)
            applySummary(summary);
        })
        .catch((e) => {
          if (alive && (statusVersions.get(id) ?? 0) === version)
            repos.value = repos.value.map((r) =>
              r.id === id
                ? { ...r, status_pending: false, status_error: String(e) }
                : r,
            );
        })
        .finally(() => {
          activeStatuses.delete(id);
          drainStatuses();
        });
    }
  }
  async function togglePin(repo: Repo) {
    if (pendingPins.value.includes(repo.id)) return;
    const value = !repo.pinned;
    changedMetadata(repo.id);
    pendingPins.value = [...pendingPins.value, repo.id];
    repos.value = repos.value.map((r) =>
      r.id === repo.id ? { ...r, pinned: value } : r,
    );
    try {
      await api.pin(repo.id, value);
      await loadMetadata();
    } catch (e) {
      if (alive) {
        repos.value = repos.value.map((r) =>
          r.id === repo.id ? { ...r, pinned: !value } : r,
        );
        error.value = String(e);
      }
    } finally {
      changedMetadata(repo.id);
      pendingPins.value = pendingPins.value.filter((id) => id !== repo.id);
    }
  }
  let token = state("");
  let accountMode = state<"browser" | "token">("browser");
  let browserAvailable = state<boolean | null>(null);
  let privateRepos = state(false);
  let loginCode = state<LoginCode | null>(null);
  let loginBusy = state(false);
  let loginError = state("");
  let loginMessage = state("");
  let loginSequence = 0;
  let loginWake: (() => void) | null = null;
  let collection = state<string[]>([]);
  let tags = state<string[]>([]);
  let showBranch = state(false);
  let conflictFile = state<string | null>(null);
  let pendingGit = state<{
    id: number;
    action: string;
    args: string[];
    title: string;
    description: string;
    expected: GitExpectation;
  } | null>(null);
  let message = state("");
  let newBranch = state("");
  let activeTab = state<"changes" | "history" | "pulls" | "activity">(
    "changes",
  );
  let actionResult = state("");
  let refreshing = state(false);
  let refreshInFlight = false;
  let detailError = state("");
  let fileQuery = state("");
  let selectedFile = state<string | null>(null);
  let diffCode = state("");
  let diffLoading = state(false);
  let diffReady = false;
  let diffInFlight: number | undefined;
  let diffError = state("");
  let diffSequence = 0;
  let loadFlight: Promise<void> | null = null;
  let selectionSequence = 0;
  let detailsRevision = 0;
  let searchInput = state<HTMLInputElement>();
  let page = state(1);
  const pageSize = 50;
  lifecycle.effect(() => {
    query.value;
    view.value;
    page.value = 1;
  });
  const commitDraft = derived((): DraftContext | null => {
    if (
      !current.value ||
      !gitState.value ||
      gitState.value.branch === "(detached)"
    )
      return null;
    return {
      target: targetOf(current.value),
      scope: { branch: gitState.value.branch, account: "", base: "" },
    };
  });
  const commitDraftKey = derived(
    () => commitDraft.value && draftKey(commitDraft.value),
  );
  let draftLoading = state(false),
    draftSequence = 0;
  const messageTooLong = derived(() => message.value.length > 500);
  lifecycle.effect(() => {
    const key = commitDraftKey.value;
    if (key) void untrack(() => restoreCommitDraft(key));
    else {
      ++draftSequence;
      message.value = "";
      draftLoading.value = false;
    }
  });
  const current = derived(
    () => repos.value.find((r) => r.id === selected.value) ?? null,
  );
  const gitState = derived(() => details.value?.git ?? null);
  const canFetch = derived(
    () =>
      !working.value &&
      !current.value?.status_error &&
      !!gitState.value?.remotes.length,
  );
  const canRestore = derived(
    () =>
      !!gitState.value &&
      !working.value &&
      !current.value?.status_error &&
      !gitState.value.dirty &&
      !gitState.value.operation &&
      !gitState.value.conflicts.length,
  );
  const canStash = derived(
    () =>
      !!gitState.value?.has_head &&
      gitState.value.branch !== "(detached)" &&
      !!gitState.value.dirty &&
      !working.value &&
      !current.value?.status_error &&
      !gitState.value.operation &&
      !gitState.value.conflicts.length,
  );
  const syncAction = derived(() => {
    if (!gitState.value?.remotes.length) return null;
    const {
      branch,
      has_head,
      remote,
      upstream,
      upstream_gone,
      operation,
      conflicts,
      dirty,
      ahead,
      behind,
    } = gitState.value;
    if (
      has_head &&
      branch !== "(detached)" &&
      remote &&
      !dirty &&
      !operation &&
      !conflicts.length &&
      !upstream_gone
    ) {
      if (!upstream)
        return { action: "push", label: "Publish branch", icon: ArrowUp };
      if (ahead && behind)
        return {
          action: "merge_upstream",
          label: "Merge upstream",
          icon: ArrowLeftRight,
        };
      if (behind)
        return { action: "pull", label: "Pull changes", icon: ArrowDown };
      if (ahead)
        return { action: "push", label: "Push commits", icon: ArrowUp };
    }
    return { action: "fetch", label: "Fetch", icon: RefreshCw };
  });
  function shelfRepos(scope: string, search = "") {
    return scope === "attention"
      ? filteredRepos(repos.value, search, "all").filter((repo) =>
          repo.status_error || repo.dirty || repo.ahead || repo.behind ||
          launchFailures.value[repo.id] || repo.ci_conclusion === "failure",
        )
      : filteredRepos(repos.value, search, scope);
  }

  const visible = derived(() => orderRepos(shelfRepos(view.value, query.value)));
  const fileNeedle = derived(() => fileQuery.value.toLocaleLowerCase());
  const activeSettings = derived(() =>
    current.value ? workflowFor(current.value) : undefined,
  );
  const reviewPaths = derived(() => activeSettings.value?.review_paths ?? []);
  const changedFiles = derived(() => [
    ...new Map(
      (details.value?.changes ?? []).map((file) => [file.path, file]),
    ).values(),
  ]);
  const commitFiles = derived(() =>
    changedFiles.value
      .filter((file) =>
        selectionMode.value === "exclude"
          ? !markedFiles.value.has(file.path)
          : markedFiles.value.size === 0 || markedFiles.value.has(file.path),
      )
      .map((file) => file.path),
  );
  const commitFileSet = derived(() => new Set(commitFiles.value));
  const selectionBranch = derived(() => gitState.value?.branch);
  lifecycle.effect(() => {
    selectionBranch.value;
    markedFiles.value = new Set();
  });
  function markFile(path: string, checked: boolean) {
    const next = new Set(markedFiles.value);
    if (checked) next.add(path);
    else next.delete(path);
    markedFiles.value = next;
  }
  function fileStatus(index: string, worktree: string, conflicted: boolean) {
    const code = index + worktree;
    if (conflicted) return "Conflict";
    if (code.includes("R")) return "Renamed";
    if (code.includes("C")) return "Copied";
    if (code.includes("D")) return "Deleted";
    if (code.includes("?") || code.includes("A")) return "New";
    if (code.includes("T")) return "Type changed";
    return "Modified";
  }
  const matchingFiles = derived(() =>
    changedFiles.value.filter((file) =>
      file.path.toLocaleLowerCase().includes(fileNeedle.value),
    ),
  );
  const foldedFiles = derived(() =>
    matchingFiles.value.filter(
      (file) =>
        !gitState.value?.conflicts.includes(file.path) &&
        reviewPaths.value.some(
          (path) =>
            file.path === path ||
            file.path.startsWith(path.endsWith("/") ? path : path + "/"),
        ),
    ),
  );
  const fileMatches = derived(() =>
    showFolded.value || fileNeedle.value
      ? matchingFiles.value
      : matchingFiles.value.filter((file) => !foldedFiles.value.includes(file)),
  );
  const repoActivity = derived(() => {
    const id = current.value?.id;
    return id === undefined
      ? []
      : activities.value.filter((item) => item.repo_id === id).slice(0, 3);
  });
  const attention = derived(
    () =>
      repos.value.filter(
        (r) =>
          r.status_error ||
          r.dirty ||
          r.ahead ||
          r.behind ||
          launchFailures.value[r.id] ||
          r.ci_conclusion === "failure",
      ).length,
  );
  function targetOf(repo: Repo) {
    return { id: repo.id, path: repo.path, registration: repo.registration };
  }
  function workflowFor(repo: Repo) {
    const entry = metadata.value.get(repo.id);
    return entry?.path === repo.path && entry.registration === repo.registration
      ? entry
      : undefined;
  }
  function orderRepos(rows: Repo[]) {
    return rows
      .slice()
      .sort(
        (a, b) =>
          Number(b.pinned) - Number(a.pinned) ||
          (a.pinned
            ? (workflowFor(a)?.pin_position ?? Number.MAX_SAFE_INTEGER) -
              (workflowFor(b)?.pin_position ?? Number.MAX_SAFE_INTEGER)
            : a.name.localeCompare(b.name) || a.id - b.id),
      );
  }
  let workflowRead = 0;
  async function loadMetadata() {
    const request = ++workflowRead;
    try {
      const rows = await api.repoWorkflow();
      if (alive && request === workflowRead) {
        metadata.value = new Map(rows.map((row) => [row.id, row]));
        repoMetadataReady.value = true;
        repos.value = repos.value.map((repo) => {
          const cached = workflowFor(repo)?.cached_status;
          return !repo.branch && cached ? { ...repo, ...cached } : repo;
        });
      }
    } catch (reason) {
      if (alive && request === workflowRead) error.value = String(reason);
    }
  }
  async function loadWorkflowApp() {
    try {
      const value = await api.workflowApp();
      if (alive) {
        workflowApp.value = value;
        workflowReady.value = true;
      }
    } catch (reason) {
      if (alive) error.value = String(reason);
    }
  }
  async function restoreCommitDraft(key: string) {
    const context = commitDraft.value;
    if (!context) return;
    const revision = ++draftSequence;
    draftLoading.value = true;
    message.value = "";
    recoveredMessage.value = "";
    try {
      const value = await readDraft(context, workflowApp.value.save_drafts);
      if (revision === draftSequence && key === commitDraftKey.value)
        message.value = value?.title ?? "";
    } catch (reason) {
      if (revision === draftSequence) error.value = String(reason);
    } finally {
      if (revision === draftSequence) draftLoading.value = false;
    }
  }
  function updateDraft(value: string) {
    message.value = value;
    if (commitDraft.value)
      void saveDraft(
        commitDraft.value,
        { title: value, body: "" },
        workflowApp.value.save_drafts,
      ).catch((reason) => {
        error.value = String(reason);
      });
  }
  async function visit(repo: Repo, branch: string | null) {
    try {
      const visited = await api.visitRepo(
        targetOf(repo),
        branch === "(detached)" ? null : branch,
      );
      if (current.value?.id === repo.id && current.value.path === repo.path)
        recentBranches.value = visited;
      await loadMetadata();
    } catch (reason) {
      if (current.value?.id === repo.id) error.value = String(reason);
    }
  }
  async function readRepoContext(repo: Repo) {
    try {
      const result = await api.repoContext(targetOf(repo));
      if (current.value?.id === repo.id && current.value.path === repo.path)
        repoContext.value = result;
      return result;
    } catch (reason) {
      if (current.value?.id === repo.id) error.value = String(reason);
      return null;
    }
  }
  async function openLocation(
    repo: Repo,
    action: Parameters<typeof api.openRepoLocation>[1],
    reference: string | null = null,
  ) {
    try {
      await api.openRepoLocation(targetOf(repo), action, reference);
    } catch (reason) {
      error.value = String(reason);
    }
  }
  function openMenu(repo: Repo, file?: string) {
    const actions = file ? [
      { label: "Open with default app", action: "fileDefault" },
      { label: "Open in editor", action: "fileEditor" },
      { label: "Show in file manager", action: "fileFolder" },
    ] as const : [
      { label: "Open repository folder", action: "folder" },
      { label: "Open in editor", action: "editor" },
      { label: "Open in terminal", action: "terminal" },
    ] as const;
    return (
      <Popover>
        <PopoverTrigger asChild>
          <ShadcnButton variant="outline" aria-label={file ? "Open selected file" : "Open repository"}>
            Open
          </ShadcnButton>
        </PopoverTrigger>
        <PopoverContent align="end" className="changes-actions">
          {actions.map(({ label, action }) => (
            <PopoverClose key={action} asChild>
              <ShadcnButton variant="ghost" onClick={() => void openLocation(repo, action, file)}>
                {label}
              </ShadcnButton>
            </PopoverClose>
          ))}
        </PopoverContent>
      </Popover>
    );
  }
  async function arrangePin(repo: Repo, earlier: boolean) {
    try {
      await api.movePin(targetOf(repo), earlier);
      await loadMetadata();
    } catch (reason) {
      error.value = String(reason);
    }
  }
  const pinned = derived(() => repos.value.filter((r) => r.pinned).length);
  const collectionOrder = state<string[]>([]);
  let collectionRevision = 0;
  const collectionPending = state(false);
  const collectionEdit = state<{ kind: "rename" | "delete"; current: string; name: string } | null>(null);
  const collectionError = state("");
  const collections = derived(() => {
    const names = new Set(repos.value.flatMap((repo) => repo.collection ? [repo.collection] : []));
    const ordered = collectionOrder.value.filter((name) => names.delete(name));
    return [...ordered, ...names];
  });
  function collectionActions(name: string) {
    const names = collections.value;
    const index = names.indexOf(name);
    const previous = names[index - 1];
    const next = names[index + 1];
    function edit(kind: "rename" | "delete") {
      collectionError.value = "";
      collectionEdit.value = { kind, current: name, name };
    }
    return {
      name,
      onRename: () => edit("rename"),
      onDelete: () => edit("delete"),
      moveEarlier: previous ? () => void moveCollection(name, previous, false) : undefined,
      moveLater: next ? () => void moveCollection(name, next, true) : undefined,
    };
  }
  async function moveCollection(source: string, destination: string, after: boolean) {
    if (collectionPending.value || source === destination || !collections.value.includes(source)) return;
    const names = collections.value.filter((name) => name !== source);
    const index = names.indexOf(destination);
    if (index < 0) return;
    names.splice(index + Number(after), 0, source);
    if (names.every((name, position) => name === collections.value[position])) return;
    collectionPending.value = true;
    ++collectionRevision;
    try {
      collectionOrder.value = await api.manageCollections({ kind: "reorder", names });
    } catch (reason) {
      error.value = String(reason);
    } finally {
      collectionPending.value = false;
    }
  }
  async function saveCollection() {
    const edit = collectionEdit.value;
    if (!edit || collectionPending.value) return;
    const name = edit.kind === "rename" ? edit.name.trim() : null;
    if (name === "") return;
    collectionError.value = "";
    collectionPending.value = true;
    ++collectionRevision;
    try {
      const order = await api.manageCollections(edit);
      for (const repo of repos.value) {
        if (repo.collection === edit.current) changedMetadata(repo.id);
      }
      repos.value = repos.value.map((repo) =>
        repo.collection === edit.current ? { ...repo, collection: name } : repo,
      );
      collectionOrder.value = order;
      if (view.value === "collection:" + edit.current) {
        view.value = name ? "collection:" + name : "all";
        query.value = "";
      }
      collectionEdit.value = null;
      showNotice(edit.kind === "rename" ? "Collection renamed" : "Collection deleted");
    } catch (reason) {
      collectionError.value = String(reason);
    } finally {
      collectionPending.value = false;
    }
  }
  const overviewMode = derived(() =>
    collections.value.length ? (overviewChoice.value ?? "collections") : "all",
  );
  const showCollectionShelf = derived(() =>
    view.value === "all" && overviewMode.value === "collections",
  );
  const collectionShelf = derived(() => {
    const counts = new Map<string, number>();
    for (const repo of repos.value) {
      if (repo.collection) counts.set(repo.collection, (counts.get(repo.collection) ?? 0) + 1);
    }
    const search = query.value.trim().toLocaleLowerCase();
    return collections.value
      .filter((name) => name.toLocaleLowerCase().includes(search))
      .map((name) => [name, counts.get(name) ?? 0] as const);
  });
  const date = (sec: number) =>
    new Date(sec * 1000).toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  const short = (value: string, max = 45) =>
    value.length > max ? "…" + value.slice(-(max - 1)) : value;
  let supplementaryFlight: Promise<void> | null = null;
  function loadSupplementary() {
    if (supplementaryFlight) return;
    const orderRevision = collectionRevision;
    supplementaryFlight = Promise.allSettled([
      api.collectionOrder().then((order) => {
        if (alive && !collectionPending.value && orderRevision === collectionRevision) collectionOrder.value = order;
      }),
      api.activity().then((rows) => {
        if (alive) activities.value = rows;
      }),
      api.accounts().then((rows) => {
        if (alive) accounts.value = rows;
      }),
    ])
      .then((results) => {
        const failure = results.find((r) => r.status === "rejected");
        if (alive && failure?.status === "rejected")
          error.value = String(failure.reason);
      })
      .finally(() => (supplementaryFlight = null));
  }
  async function load() {
    loadSupplementary();
    if (loadFlight) return loadFlight;
    const revision = metadataRevision;
    loadFlight = api
      .repos()
      .then((rows) => {
        if (!alive) return;
        const previous = new Map(repos.value.map((r) => [r.id, r]));
        repos.value = rows.map((row) => {
          const old = previous.get(row.id);
          if (
            !old ||
            old.path !== row.path ||
            old.registration !== row.registration
          ) {
            const cached = workflowFor(row)?.cached_status;
            return cached ? { ...row, ...cached } : row;
          }
          const git = {
            branch: old.branch,
            dirty: old.dirty,
            ahead: old.ahead,
            behind: old.behind,
            conflicts: old.conflicts,
            missing: old.missing,
            status_error: old.status_error,
            status_pending: old.status_pending,
          };
          const local =
            (localRevisions.get(row.id) ?? 0) > revision ||
            pendingPins.value.includes(row.id)
              ? {
                  pinned: old.pinned,
                  collection: old.collection,
                  tags: old.tags,
                }
              : {};
          return { ...row, ...git, ...local };
        });
        requestStatuses(repos.value.map((r) => r.id));
      })
      .catch((e) => {
        if (alive) error.value = String(e);
      })
      .finally(() => {
        if (alive) loading.value = false;
        loadFlight = null;
      });
    return loadFlight;
  }
  function beginSelection(id: number) {
    const sequence = ++selectionSequence;
    selected.value = id;
    message.value = "";
    newBranch.value = "";
    details.value = null;
    detailError.value = "";
    activeTab.value = "changes";
    actionResult.value = "";
    repoContext.value = null;
    recentBranches.value = [];
    recoveredMessage.value = "";
    gitFailure.value = null;
    showFolded.value = false;
    showFileSearch.value = false;
    markedFiles.value = new Set();
    fileQuery.value = "";
    selectedFile.value = null;
    diffCode.value = "";
    diffError.value = "";
    diffLoading.value = false;
    diffReady = false;
    diffInFlight = undefined;
    ++diffSequence;
    return sequence;
  }
  async function readDetails(id: number) {
    const selection = selectionSequence;
    const revision = ++detailsRevision;
    const isCurrent = () =>
      alive &&
      selected.value === id &&
      selection === selectionSequence &&
      revision === detailsRevision;
    try {
      const result = await api.details(id);
      if (!isCurrent()) return null;
      const paths = new Set(result.changes.map((file) => file.path));
      const file =
        !details.value ||
        (selectedFile.value !== null && !paths.has(selectedFile.value))
          ? (result.changes[0]?.path ?? null)
          : selectedFile.value;
      if ([...markedFiles.value].some((path) => !paths.has(path))) {
        markedFiles.value = new Set(
          [...markedFiles.value].filter((path) => paths.has(path)),
        );
      }
      details.value = result;
      detailError.value = "";
      updateRepoStatus(id, result);
      if (activeTab.value === "changes") {
        if (paths.size) {
          if (file !== selectedFile.value || diffInFlight === undefined) {
            void showDiff(file, id, file === selectedFile.value && diffReady);
          }
        } else {
          ++diffSequence;
          selectedFile.value = null;
          diffCode.value = "";
          diffError.value = "";
          diffLoading.value = false;
          diffReady = false;
          diffInFlight = undefined;
        }
      }
      return result;
    } catch (reason) {
      if (isCurrent()) detailError.value = String(reason);
      return null;
    }
  }
  async function selectRepo(id: number, tab: "changes" | "activity" = "changes") {
    beginSelection(id);
    activeActivityId.value = null;
    activeTab.value = tab;
    const repo = repos.value.find((repo) => repo.id === id);
    if (repo) void readRepoContext(repo);
    const result = await readDetails(id);
    if (result && repo) void visit(repo, result.git.branch);
  }
  function openWorkspaceActivity(id: number | null = null) {
    selected.value = null;
    activeActivityId.value = id;
    view.value = "activity";
  }

  function openRepositoryActivity(id: number, activityId?: number) {
    if (selected.value !== id) void selectRepo(id, "activity");
    else activeTab.value = "activity";
    activeActivityId.value = activityId ?? activities.value.find((item) => item.repo_id === id && item.kind === "ci")?.id ?? null;
  }

  async function act(
    fn: () => Promise<unknown>,
    msg: string,
    refreshWorkspace = true,
  ) {
    if (working.value) return false;
    const actionRepo = selected.value;
    working.value = true;
    error.value = "";
    ++detailsRevision;
    if (actionRepo !== null)
      statusVersions.set(actionRepo, (statusVersions.get(actionRepo) ?? 0) + 1);
    try {
      const result = await fn();
      showNotice(msg);
      if (selected.value === actionRepo)
        actionResult.value = typeof result === "string" ? result : "";
      if (refreshWorkspace) await load();
      else
        void api
          .activity()
          .then((rows) => (activities.value = rows))
          .catch(() => {});
      if (selected.value !== null) await readDetails(selected.value);
      return true;
    } catch (reason) {
      error.value = String(reason);
      if (refreshWorkspace) await load();
      if (selected.value !== null) await readDetails(selected.value);
      return false;
    } finally {
      working.value = false;
      drainStatuses();
    }
  }
  async function repositoryAdded(repo: Repo, warning?: string | null) {
    showAdd.value = false;
    showNotice("Repository ready");
    error.value = warning ?? "";
    await load();
    if (!repos.value.some((r) => r.id === repo.id))
      repos.value = [...repos.value, repo];
    await selectRepo(repo.id);
  }
  function openAdd() {
    showAdd.value = true;
  }
  function closeAdd() {
    showAdd.value = false;
  }
  async function prepareTour(target: string) {
    const repo = repos.value.find((repo) => repo.id === selected.value && !repo.missing)
      ?? repos.value.find((repo) => !repo.missing);
    if (repo && target !== "add") {
      if (selected.value !== repo.id) await selectRepo(repo.id);
      activeTab.value = "changes";
    } else {
      selected.value = null;
      view.value = "all";
    }
    await tick();
  }
  function updateRepoStatus(id: number, result: RepoDetails) {
    statusVersions.set(id, (statusVersions.get(id) ?? 0) + 1);
    repos.value = repos.value.map((repo) =>
      repo.id === id
        ? {
            ...repo,
            branch: result.git.branch,
            dirty: result.git.dirty,
            ahead: result.git.ahead,
            behind: result.git.behind,
            conflicts: result.git.conflicts.length,
            missing: false,
            status_error: null,
            status_pending: false,
          }
        : repo,
    );
  }
  async function refresh(background = false) {
    if (refreshInFlight || working.value) return;
    refreshInFlight = true;
    if (!background) refreshing.value = true;
    try {
      if (selected.value === null) await load();
      else if (details.value || detailError.value)
        await readDetails(selected.value);
    } finally {
      refreshInFlight = false;
      refreshing.value = false;
    }
  }
  function startupRepositories() {
    return orderRepos(repos.value).map((repo) => ({
      ...repo,
      fetch_mode: workflowFor(repo)?.fetch_mode,
    }));
  }
  async function fetchPreferencesSaved(settings: FetchSettings) {
    launchPreferences.value = settings;
    await loadMetadata();
  }
  async function toggleRepoFetch(repo: Repo, enabled: boolean) {
    if (savingRepoFetch.value.has(repo.id)) return;
    savingRepoFetch.value.add(repo.id);
    try {
      await api.saveRepoFetchMode(targetOf(repo), enabled ? "on" : "off");
      await loadMetadata();
    } catch (reason) {
      toast.error(String(reason));
    } finally {
      savingRepoFetch.value.delete(repo.id);
    }
  }
  async function fetchOnLaunch() {
    try {
      const settings = await api.fetchSettings();
      if (!alive) return;
      launchPreferences.value = settings;
      if (!settings.enabled) return;
      await load();
      await tick();
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      );
      if (!alive) return;
      await loadMetadata();
      const batch = ++launchBatch;
      await launchFetch(
        repos.value.map((repo) => ({
          ...repo,
          fetch_mode: workflowFor(repo)?.fetch_mode,
        })),
        settings.scope,
        (id) => launchRepo(id, batch),
      );
      await loadMetadata();
    } catch (reason) {
      if (alive) error.value = String(reason);
    }
  }
  async function launchRepo(id: number, batch: number) {
    if (!alive) return;
    try {
      if (await api.launchFetch(id, batch)) requestStatuses([id]);
      delete launchFailures.value[id];
    } catch (reason) {
      launchFailures.value[id] = String(reason);
    }
  }
  async function retryLaunchFetch(repository?: number) {
    if (retryingLaunchFetch.value) return;
    retryingLaunchFetch.value = true;
    try {
      const batch = ++launchBatch;
      await launchFetch(
        repos.value.filter(
          (repo) =>
            launchFailures.value[repo.id] &&
            (repository === undefined || repo.id === repository),
        ),
        "all",
        (id) => launchRepo(id, batch),
      );
      await loadMetadata();
    } finally {
      retryingLaunchFetch.value = false;
    }
  }
  function openOrganization(repo?: Repo, group?: string) {
    if (working.value) return;
    addingCollection.value = !repo;
    organizationId.value = repo?.id ?? current.value?.id ?? null;
    existingCollection.value = group ?? null;
    collectionName.value = group ?? "";
    collectionQuery.value = "";
    const currentRepo = current.value;
    collectionRepos.value = new Set(!repo && currentRepo && !group ? [currentRepo.id] : []);
    organizationError.value = "";
    collection.value = repo?.collection ? [repo.collection] : [];
    tags.value = repo ? [...repo.tags] : [];
    showOrganize.value = true;
  }
  async function organize() {
    const repo = organizationRepo.value;
    const adding = addingCollection.value;
    const group = adding ? collectionName.value.trim() : (collection.value[0] ?? "");
    const chosen = adding ? repos.value.filter((candidate) => collectionRepos.value.has(candidate.id)) : [];
    if (organizing.value || (adding ? !group || !chosen.length : !repo)) return;
    const values = [...tags.value];
    organizing.value = true;
    organizationError.value = "";
    try {
      if (adding) {
        await api.assignCollection(chosen.map(targetOf), group);
      } else if (repo) {
        await api.organize(repo.id, group, values);
      }
      if (!alive) return;
      const updatedIds = new Set(adding ? chosen.map((item) => item.id) : [repo?.id]);
      for (const id of updatedIds) if (id !== undefined) changedMetadata(id);
      repos.value = repos.value.map((item) =>
        updatedIds.has(item.id)
          ? { ...item, collection: group || null, tags: adding ? item.tags : values }
          : item,
      );
      showOrganize.value = false;
      if (adding) {
        selected.value = null;
        view.value = "collection:" + group;
        query.value = "";
      }
      showNotice("Collection updated");
    } catch (reason) {
      if (alive) organizationError.value = String(reason);
    } finally {
      organizing.value = false;
    }
  }
  function openAccount() {
    accountMode.value = "browser";
    privateRepos.value = false;
    loginError.value = "";
    loginMessage.value = "";
    showAccount.value = true;
  }
  function closeAccount() {
    ++loginSequence;
    showAccount.value = false;
    token.value = "";
    loginError.value = "";
    loginMessage.value = "";
    loginWake?.();
    const id = loginCode.value?.id;
    loginCode.value = null;
    if (id !== undefined)
      void api.cancelLogin(id).catch(() => {
        error.value =
          "Could not cancel GitHub sign-in. Restart Reponiq before trying again.";
      });
  }
  function waitForLogin(ms: number) {
    return new Promise<void>((resolve) => {
      const timer = setTimeout(() => {
        loginWake = null;
        resolve();
      }, ms);
      loginWake = () => {
        clearTimeout(timer);
        loginWake = null;
        resolve();
      };
    });
  }
  async function reopenLogin() {
    if (!loginCode.value) return;
    try {
      await api.openLogin(loginCode.value.id);
      loginError.value = "";
    } catch (e) {
      loginError.value = String(e);
    }
  }
  async function browserConnect() {
    if (loginBusy.value || working.value || !browserAvailable.value) return;
    const sequence = ++loginSequence;
    loginBusy.value = true;
    loginError.value = "";
    token.value = "";
    let id: number | undefined;
    try {
      const code = await api.startLogin(privateRepos.value);
      id = code.id;
      if (sequence !== loginSequence || !showAccount.value) {
        await api.cancelLogin(code.id);
        return;
      }
      loginCode.value = code;
      await reopenLogin();
      await waitForLogin(code.interval * 1000);
      while (sequence === loginSequence && showAccount.value) {
        const result = await api.pollLogin(code.id);
        if (sequence !== loginSequence || !showAccount.value) break;
        if (result.status === "connected") {
          showNotice("Connected @" + result.account.login);
          loginCode.value = null;
          closeAccount();
          await load();
          return;
        }
        loginMessage.value = result.message ?? "Waiting for GitHub…";
        await waitForLogin(Math.max(1, result.retry_after) * 1000);
      }
    } catch (e) {
      if (sequence === loginSequence && showAccount.value) {
        loginError.value = String(e);
        loginCode.value = null;
      }
    } finally {
      if (id !== undefined) void api.cancelLogin(id).catch(() => {});
      loginBusy.value = false;
      void load();
    }
  }
  async function connect() {
    if (!token.value.trim() || loginBusy.value) return;
    if (
      await act(
        () => api.connect(token.value.trim()),
        "GitHub account connected",
      )
    )
      closeAccount();
  }
  async function sync() {
    await act(async () => {
      const results = [];
      for (const a of accounts.value) results.push(await api.sync(a.id));
      return results.join("\n");
    }, "GitHub workflows synchronized");
  }
  async function runGit(action: string, args: string[] = []) {
    if (!current.value || working.value) return false;
    const repo = current.value,
      expected = gitState.value
        ? {
            branch: gitState.value.branch,
            head: gitState.value.head,
            path: current.value.path,
            registration: current.value.registration,
            author:
              action === "commit_paths"
                ? (repoContext.value?.author ?? undefined)
                : undefined,
          }
        : null;
    gitFailure.value = null;
    const ok = await act(
      async () => {
        try {
          return await api.git(repo.id, action, args, expected);
        } catch (reason) {
          if (reason instanceof GitFailure)
            gitFailure.value = {
              code: reason.code,
              branch: action === "switch_branch" ? args[0] : null,
            };
          throw reason;
        }
      },
      action === "commit_paths"
        ? "Commit created"
        : `Git ${action.replaceAll("_", " ")} completed`,
      false,
    );
    if (["fetch", "pull", "merge_upstream"].includes(action))
      void loadMetadata();
    if (ok && action === "fetch") delete launchFailures.value[repo.id];
    if (
      ok &&
      action === "stash_changes" &&
      current.value?.id === repo.id &&
      current.value.path === repo.path &&
      current.value.registration === repo.registration
    )
      markedFiles.value = new Set();
    if (
      ok &&
      ["switch_branch", "create_branch"].includes(action) &&
      current.value?.id === repo.id
    ) {
      void visit(repo, gitState.value?.branch ?? null);
      void readRepoContext(repo);
    }
    return ok;
  }
  async function finishCommit(text: string, files: string[]) {
    const context = commitDraft.value,
      key = commitDraftKey.value,
      sequence = selectionSequence;
    if (await runGit("commit_paths", [text, ...files])) {
      if (context && (commitDraftKey.value !== key || message.value === text))
        await saveDraft(context, null, workflowApp.value.save_drafts).catch(
          (reason) => {
            error.value = String(reason);
          },
        );
      if (commitDraftKey.value === key && message.value === text)
        message.value = "";
      if (selectionSequence === sequence) markedFiles.value = new Set();
    }
  }
  function commit() {
    if (
      !current.value ||
      !gitState.value ||
      working.value ||
      draftLoading.value ||
      messageTooLong.value ||
      !message.value.trim() ||
      !commitFiles.value.length
    )
      return;
    const request = {
      repo: current.value,
      branch: gitState.value.branch,
      head: gitState.value.head,
      message: message.value,
      files: [...commitFiles.value],
    };
    const sequence = selectionSequence;
    const lastAuthor = repoContext.value?.author;
    void readRepoContext(current.value).then((context) => {
      if (
        !context ||
        sequence !== selectionSequence ||
        current.value?.path !== request.repo.path ||
        current.value.registration !== request.repo.registration ||
        gitState.value?.branch !== request.branch ||
        gitState.value.head !== request.head ||
        working.value
      )
        return;
      if (
        request.message !== message.value ||
        request.files.length !== commitFiles.value.length ||
        request.files.some((file, index) => file !== commitFiles.value[index])
      ) {
        error.value =
          "The commit selection or message changed. Review it and click Commit again.";
        return;
      }
      if (!context.author) {
        error.value =
          "Git needs a commit author. Open Repository settings to set a name and email.";
        return;
      }
      if (lastAuthor !== context.author) {
        error.value = "Review the current Git author before committing.";
        return;
      }
      const override = activeSettings.value?.warn_default;
      if (
        context.default_branch === request.branch &&
        (override === "on" ||
          (override !== "off" && workflowApp.value.warn_default))
      ) {
        branchNameForCommit.value = "";
        defaultCommit.value = request;
      } else void finishCommit(request.message, request.files);
    });
  }
  async function acceptDefaultCommit(create: boolean) {
    const task = defaultCommit.value;
    if (
      !task ||
      task.repo.id !== current.value?.id ||
      task.repo.registration !== current.value.registration ||
      task.repo.path !== current.value.path ||
      task.branch !== gitState.value?.branch ||
      task.head !== gitState.value.head
    )
      return;
    defaultCommit.value = null;
    if (!create) {
      await finishCommit(task.message, task.files);
      return;
    }
    if (
      (await runGit("create_branch", [branchNameForCommit.value])) &&
      current.value?.id === task.repo.id
    ) {
      await tick();
      if (commitDraftKey.value) await restoreCommitDraft(commitDraftKey.value);
      if (message.value && message.value !== task.message)
        recoveredMessage.value = task.message;
      else updateDraft(task.message);
    }
  }
  async function undoCommit() {
    if (!undoConfirm.value || working.value) return;
    const { repo, branch, head } = undoConfirm.value;
    const expected = { branch, head };
    undoConfirm.value = null;
    let text = "";
    if (
      (await act(
        async () => {
          text = await api.undoCommit(targetOf(repo), expected);
        },
        "Commit undone. Files remain staged.",
        false,
      )) &&
      current.value?.id === repo.id &&
      gitState.value?.branch === expected.branch
    ) {
      if ((message.value && message.value !== text) || text.length > 500)
        recoveredMessage.value = text;
      else updateDraft(text);
      activeTab.value = "changes";
    }
  }
  async function createBranch() {
    const id = selected.value;
    const name = newBranch.value;
    if (
      (await runGit("create_branch", [name])) &&
      selected.value === id &&
      newBranch.value === name
    )
      newBranch.value = "";
  }
  function revertCommit(hash: string, merge: boolean) {
    queueGit(
      "revert",
      "Revert commit?",
      "Create a new commit that undoes " +
        hash.slice(0, 8) +
        (merge ? " relative to its first parent" : "") +
        ". Existing history will be preserved.",
      [hash],
    );
  }
  function queueGit(
    action: string,
    title: string,
    description: string,
    args: string[] = [],
  ) {
    if (!current.value || !gitState.value || working.value) return;
    pendingGit.value = {
      id: current.value.id,
      action,
      args,
      title,
      description,
      expected: {
        branch: gitState.value.branch,
        head: gitState.value.head,
        path: current.value.path,
        registration: current.value.registration,
      },
    };
  }
  async function executePendingGit() {
    if (!pendingGit.value) return;
    const task = pendingGit.value;
    pendingGit.value = null;
    gitFailure.value = null;
    await act(
      async () => {
        try {
          return await api.git(task.id, task.action, task.args, task.expected);
        } catch (reason) {
          if (reason instanceof GitFailure)
            gitFailure.value = { code: reason.code, branch: null };
          throw reason;
        }
      },
      task.action === "revert"
        ? "Revert commit created"
        : "Git action completed",
      false,
    );
    if (["pull", "merge_upstream"].includes(task.action)) void loadMetadata();
  }
  function openRemoveRepo(repo: Repo) {
    repoDialogError.value = "";
    removing.value = { repo, deleteFolder: false };
  }

  async function manageRepository(action: "rename" | "remove") {
    const rename = renaming.value;
    const remove = removing.value;
    if (working.value || (action === "rename" ? !rename : !remove)) return;
    repoDialogError.value = "";
    const success = await act(async () => {
      if (action === "rename" && rename) {
        await api.renameRepo(targetOf(rename.repo), rename.name.trim());
        return;
      }
      if (!remove) return;
      const { repo, deleteFolder } = remove;
      await api.remove(targetOf(repo), deleteFolder);
      forgetRepositoryDrafts(repo);
      delete launchFailures.value[repo.id];
      if (selected.value === repo.id) {
        selected.value = null;
        details.value = null;
        message.value = "";
        diffLoading.value = false;
        ++diffSequence;
      }
    }, action === "rename" ? "Repository renamed" : "Repository removed");
    if (success) {
      renaming.value = null;
      removing.value = null;
    } else {
      repoDialogError.value = error.value;
    }
  }
  async function disconnectAccount(account: Account) {
    if (
      await confirm({
        title: `Disconnect @${account.login}?`,
        description: "Reponiq will delete the saved token from this device.",
        action: "Disconnect",
        destructive: true,
      })
    ) {
      await act(() => api.disconnect(account.id), "Account disconnected");
    }
  }
  async function useRecoveredMessage() {
    const text = recoveredMessage.value,
      key = commitDraftKey.value;
    if (
      message.value &&
      !(await confirm({
        title: "Replace the current draft?",
        action: "Use recovered message",
      }))
    )
      return;
    if (key === commitDraftKey.value && text === recoveredMessage.value) {
      updateDraft(text);
      recoveredMessage.value = "";
    }
  }
  async function showDiff(
    file: string | null,
    id = selected.value,
    background = false,
  ) {
    if (id === null || (background && diffInFlight !== undefined)) return;
    const sequence = ++diffSequence;
    diffInFlight = sequence;
    if (file !== selectedFile.value) {
      selectedFile.value = file;
      diffCode.value = "";
      diffError.value = "";
      diffReady = false;
    }
    if (!background && !diffReady) diffLoading.value = true;
    try {
      const code = await api.git(id, "diff", file === null ? [] : [file]);
      if (sequence === diffSequence && selected.value === id) {
        diffCode.value = code;
        diffReady = true;
        diffError.value = "";
      }
    } catch (e) {
      if (sequence === diffSequence && selected.value === id)
        diffError.value = String(e);
    } finally {
      if (diffInFlight === sequence) diffInFlight = undefined;
      if (sequence === diffSequence) diffLoading.value = false;
    }
  }
  function modalOpen() {
    return (
      document.querySelector(
        '[data-slot="dialog-content"][aria-modal="true"]',
      ) !== null
    );
  }
  async function keyboard(event: KeyboardEvent) {
    const editing =
      event.target instanceof HTMLInputElement ||
      event.target instanceof HTMLTextAreaElement;
    if (event.defaultPrevented || event.isComposing || modalOpen()) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      if (!working.value) showQuick.value = true;
    }
    if (
      (event.ctrlKey || event.metaKey) &&
      event.key.toLowerCase() === "f" &&
      current.value &&
      activeTab.value === "changes"
    ) {
      event.preventDefault();
      showFileSearch.value = true;
      await tick();
      fileSearch.value?.focus();
    }
    if (
      event.key === "Escape" &&
      !editing &&
      !showAdd.value &&
      !showAccount.value &&
      !showOrganize.value &&
      !showBranch.value &&
      !conflictFile.value
    ) {
      selected.value = null;
      details.value = null;
    }
  }
  function confirmGit(action: string) {
    const labels: {
      [k: string]: [string, string];
    } = {
      pull: [
        "Pull changes?",
        "Fetch and fast-forward this branch from its upstream.",
      ],
      push: gitState.value?.upstream
        ? ["Push commits?", "Push the committed branch to its tracking remote."]
        : ["Publish branch?", "Push this branch and set up remote tracking."],
      merge_upstream: [
        "Merge upstream?",
        "Combine local and remote commits without rewriting history.",
      ],
      abort_operation: [
        "Abort Git operation?",
        "Resolution work from this operation may be discarded. Git will attempt to restore the pre-operation state.",
      ],
      unstage_all: [
        "Unstage all files?",
        "Keep the working files and remove them from the next commit.",
      ],
    };
    const label = labels[action];
    if (label) queueGit(action, ...label);
    else void runGit(action);
  }
  lifecycle.mount(() => {
    void load();
    void loadMetadata();
    void loadWorkflowApp();
    const media = matchMedia("(prefers-color-scheme: dark)");
    const readSystemTheme = () => {
      systemDark.value = media.matches;
    };
    readSystemTheme();
    media.addEventListener("change", readSystemTheme);
    void loadAppearance();
    void api
      .workspaceSettings()
      .then((settings) => {
        if (alive) updateWorkspace(settings);
      })
      .catch((e) => {
        if (alive) error.value = String(e);
      });
    void api
      .browserLoginAvailable()
      .then((value) => {
        if (alive) browserAvailable.value = value;
      })
      .catch(() => (browserAvailable.value = false));
    const idle = () =>
      document.visibilityState === "visible" && !working.value && !modalOpen();
    const timer = setInterval(() => {
      if (idle()) {
        void load();
        void scanDefault();
      }
    }, 60000);
    const statusTimer = setInterval(() => {
      if (idle()) {
        if (selected.value !== null) void refresh(true);
        else
          requestStatuses(
            visible.value
              .slice((page.value - 1) * pageSize, page.value * pageSize)
              .map((r) => r.id),
          );
      }
    }, 10000);
    let lastFocus = 0;
    const focus = () => {
      if (idle() && Date.now() - lastFocus > 2000) {
        lastFocus = Date.now();
        if (selected.value !== null) void refresh(true);
        else void load();
      }
    };
    window.addEventListener("focus", focus);
    document.addEventListener("visibilitychange", focus);
    return () => {
      alive = false;
      statusQueue.clear();
      clearTimeout(noticeTimer);
      media.removeEventListener("change", readSystemTheme);
      clearInterval(timer);
      clearInterval(statusTimer);
      window.removeEventListener("focus", focus);
      document.removeEventListener("visibilitychange", focus);
      closeAccount();
    };
  });
  return () => {
    const currentRepo = current.value;
    return (
      <>
        <WindowEvents onKeyDown={keyboard} />
        <a href={"#main-content"} className={cn("skip-link")}>
          {"Skip to workspace"}
        </a>
        <AppUpdates busy={working.value}>
          <div className={cn("app-shell")}>
            <Sidebar>
              <SidebarHeader className="workspace-sidebar-header">
                <div className={cn("brand")}>
                  <div className={cn("brand-mark")}>
                    <svg
                      width={"24"}
                      height={"24"}
                      viewBox={"0 0 24 24"}
                      fill={"none"}
                      aria-hidden={"true"}
                    >
                      <path
                        d={"M6 19V5h7a4 4 0 0 1 0 8H6m7 0 5 6"}
                        stroke={"currentColor"}
                        strokeWidth={"2.5"}
                        strokeLinecap={"round"}
                        strokeLinejoin={"round"}
                      />
                      <circle
                        cx={"18"}
                        cy={"19"}
                        r={"2"}
                        fill={"currentColor"}
                        stroke={"none"}
                      />
                    </svg>
                  </div>
                  <div>
                    <div className={cn("brand-title")}>{"Reponiq"}</div>
                  </div>
                </div>
              </SidebarHeader>
              <SidebarGroup className="workspace-sidebar-group">
                <div className={cn("side-section")}>{"WORKSPACE"}</div>
                <SidebarMenu>
                  <SidebarMenuButton
                    title={"Overview"}
                    active={view.value === "all" && selected.value === null}
                    onClick={() => {
                      selected.value = null;
                      view.value = "all";
                    }}
                  >
                    <LayoutDashboard size={17} />
                    {" Overview"}
                  </SidebarMenuButton>
                  <SidebarMenuButton
                    title={"Pinned repositories"}
                    active={view.value === "pinned" && selected.value === null}
                    onClick={() => {
                      selected.value = null;
                      view.value = "pinned";
                    }}
                  >
                    <Pin
                      size={17}
                      fill={
                        view.value === "pinned" && selected.value === null
                          ? "currentColor"
                          : "none"
                      }
                    />
                    {" Pinned "}
                    <span className={cn("side-count")}>{pinned.value}</span>
                  </SidebarMenuButton>
                  <SidebarMenuButton
                    title={"Needs attention"}
                    active={
                      view.value === "attention" && selected.value === null
                    }
                    onClick={() => {
                      selected.value = null;
                      view.value = "attention";
                    }}
                  >
                    <CircleAlert size={17} />
                    {" Needs attention "}
                    <span className={cn("side-count")}>{attention.value}</span>
                  </SidebarMenuButton>
                  <SidebarMenuButton
                    title={"Activity"}
                    data-tour="activity"
                    active={
                      view.value === "activity" && selected.value === null
                    }
                    onClick={() => openWorkspaceActivity()}
                  >
                    <History size={17} />
                    {" Activity"}
                  </SidebarMenuButton>
                </SidebarMenu>
              </SidebarGroup>
              <SidebarGroup className="workspace-sidebar-group">
                <div className={cn("side-head")}>
                  <span className={cn("side-section")}>{"COLLECTIONS"}</span>
                  <Button
                    data-tour="collections"
                    title={"Add collection"}
                    aria-label={"Add collection"}
                    disabled={working.value}
                    onClick={() => openOrganization()}
                    className={cn("icon-only")}
                  >
                    <Plus size={15} />
                  </Button>
                </div>
                <SidebarMenu>
                  {(() => {
                    const members = new Map<string, Repo[]>();
                    for (const repo of repos.value) {
                      if (!repo.collection) continue;
                      const entries = members.get(repo.collection);
                      if (entries) entries.push(repo);
                      else members.set(repo.collection, [repo]);
                    }
                    return collections.value.map((name) => (
                      <CollectionMenuItem
                        key={name}
                        name={name}
                        actions={collectionActions(name)}
                        onMove={moveCollection}
                        repositories={orderRepos(members.get(name) ?? [])}
                        active={view.value === "collection:" + name && selected.value === null}
                        selectedRepo={selected.value}
                        onOpen={() => {
                          selected.value = null;
                          view.value = "collection:" + name;
                        }}
                        onRepository={(id) => void selectRepo(id)}
                      />
                    ));
                  })()}
                </SidebarMenu>
                {collections.value.length === 0 ? (
                  <>
                    <div className={cn("muted-sidebar")}>
                      {"Organize repositories into groups"}
                    </div>
                  </>
                ) : null}
              </SidebarGroup>
              <div className={cn("sidebar-grow")} />
              <SidebarFooter className={cn("sidebar-bottom")}>
                {appearanceReady.value &&
                setupStep.value !== "complete" &&
                !showSetup.value ? (
                  <>
                    <Button
                      title={"Resume setup"}
                      onClick={() => (showSetup.value = true)}
                    >
                      <Settings size={17} />
                      {" Resume setup"}
                    </Button>
                  </>
                ) : null}
                <SidebarMenu>
                  <SidebarMenuButton
                    title={"Settings"}
                    active={
                      view.value === "settings" && selected.value === null
                    }
                    onClick={() => {
                      selected.value = null;
                      view.value = "settings";
                    }}
                  >
                    <Settings size={17} />
                    {" Settings"}
                  </SidebarMenuButton>
                </SidebarMenu>
                <AppUpdateButton />
                <div className={cn("connection")}>
                  <span className={cn("live-dot")} />
                  {" Local workspace "}
                  <span className={cn("version")}>
                    {"v"}
                    {appVersion}
                  </span>
                </div>
              </SidebarFooter>
            </Sidebar>
            <div className={cn("main-area")}>
              <main
                id={"main-content"}
                tabIndex={-1}
                aria-busy={loading.value || working.value}
                className={cn("content", !!currentRepo && "workbench-content")}
              >
                {error.value ? (
                  <>
                    <div role={"alert"} className={cn("alert error")}>
                      <CircleAlert size={16} />
                      <span>{error.value}</span>
                      {gitFailure.value?.code === "upstream" &&
                      details.value ? (
                        <>
                          <Button
                            variant={"secondary"}
                            onClick={() => (showBranches.value = true)}
                          >
                            {"Choose branch"}
                          </Button>
                        </>
                      ) : (
                        <>
                          {gitFailure.value?.code === "missing" &&
                          currentRepo ? (
                            <>
                              <Button
                                variant={"secondary"}
                                onClick={() => (locating.value = currentRepo)}
                              >
                                {"Locate folder"}
                              </Button>
                            </>
                          ) : (
                            <>
                              {gitFailure.value?.code === "conflict" &&
                              currentRepo ? (
                                <>
                                  <Button
                                    variant={"secondary"}
                                    onClick={() =>
                                      (activeTab.value = "changes")
                                    }
                                  >
                                    {"Review changes"}
                                  </Button>
                                </>
                              ) : (
                                <>
                                  {gitFailure.value?.code === "stale" &&
                                  currentRepo ? (
                                    <>
                                      <Button
                                        variant={"secondary"}
                                        onClick={() => refresh()}
                                      >
                                        {"Refresh repository"}
                                      </Button>
                                    </>
                                  ) : null}
                                </>
                              )}
                            </>
                          )}
                        </>
                      )}
                      <Button
                        title={"Dismiss error"}
                        onClick={() => {
                          error.value = "";
                          gitFailure.value = null;
                        }}
                      >
                        <X size={15} />
                      </Button>
                    </div>
                  </>
                ) : null}
                {Object.keys(launchFailures.value).length ? (
                  <>
                    <div role={"status"} className={cn("alert error")}>
                      <CircleAlert size={16} />
                      <span>
                        {Object.keys(launchFailures.value).length}
                        {" repositories need help fetching."}
                      </span>
                      <Button
                        variant={"secondary"}
                        disabled={working.value}
                        onClick={() => {
                          selected.value = null;
                          details.value = null;
                          view.value = "attention";
                          query.value = "";
                        }}
                      >
                        {"View affected repositories"}
                      </Button>
                    </div>
                  </>
                ) : null}
                {currentRepo ? (
                  <>
                    <div className={cn("workbench-shell")}>
                      <section
                        aria-label={"Git workbench"}
                        className={cn("workbench-main")}
                      >
                        <div className={cn("workbench-header", activeTab.value === "changes" && "changes-header")}>
                          <div className={cn("page-heading workbench-heading")}>
                            <div>
                              <Button
                                onClick={() => {
                                  selected.value = null;
                                  details.value = null;
                                }}
                                className={cn("back-link")}
                              >
                                <ArrowLeftRight size={14} />
                                {" All repositories"}
                              </Button>
                              <div className={cn("heading-row")}>
                                <PlatformIcon
                                  remote={currentRepo.remote_url}
                                  size={25}
                                />
                                <h1>{currentRepo.name}</h1>
                              </div>
                              <p title={currentRepo.path}>{currentRepo.path}</p>
                            </div>
                          </div>
                          <div className="repo-command-bar" role="group" aria-label="Repository controls">
                            <ShadcnButton
                              variant="ghost"
                              size="icon"
                              aria-label={
                                currentRepo.pinned
                                  ? "Unpin repository"
                                  : "Pin repository"
                              }
                              disabled={pendingPins.value.includes(
                                currentRepo.id,
                              )}
                              onClick={() => togglePin(currentRepo)}
                            >
                              <Pin
                                size={16}
                                fill={
                                  currentRepo.pinned ? "currentColor" : "none"
                                }
                              />
                            </ShadcnButton>
                            <ShadcnButton
                              data-tour="organize"
                              variant="outline"
                              disabled={working.value}
                              onClick={() => openOrganization(currentRepo)}
                            >
                              <Tags size={15} />
                              {" Organize"}
                            </ShadcnButton>
                            {launchPreferences.value && (
                              <ShadcnField
                                orientation="horizontal"
                                className="repo-fetch-switch"
                                title={launchPreferences.value.enabled ? undefined : "Turn on Fetch on launch in Settings to use this preference."}
                              >
                                <Switch
                                  id="repository-fetch-on-launch"
                                  checked={repoFetchEnabled(
                                    { ...currentRepo, fetch_mode: workflowFor(currentRepo)?.fetch_mode },
                                    launchPreferences.value.scope,
                                  )}
                                  disabled={!repoMetadataReady.value || savingRepoFetch.value.has(currentRepo.id)}
                                  onCheckedChange={(enabled) => void toggleRepoFetch(currentRepo, enabled)}
                                />
                                <FieldLabel htmlFor="repository-fetch-on-launch">Fetch on launch</FieldLabel>
                              </ShadcnField>
                            )}
                            <ShadcnButton
                              aria-label={
                                "Switch branch. Current branch: " +
                                currentRepo.branch
                              }
                              disabled={working.value || !details.value}
                              onClick={() => (showBranches.value = true)}
                              variant="outline"
                              className="current-branch"
                              data-tour="branches"
                            >
                              <GitBranch size={16} />
                              <span>{currentRepo.branch}</span>
                            </ShadcnButton>
                            <div
                              role="group"
                              aria-label="Repository actions"
                              data-tour="sync"
                              className="repo-sync-buttons"
                            >
                              {(() => {
                                const actions = [
                                  ...(details.value?.saved_changes.length ? [{
                                    label: `Saved changes (${details.value.saved_changes.length})`,
                                    disabled: working.value,
                                    select: () => (showSavedChanges.value = true),
                                  }] : []),
                                  ...(canStash.value ? [{
                                    label: "Stash changes",
                                    disabled: false,
                                    select: () => runGit("stash_changes"),
                                  }] : []),
                                  ...(syncAction.value && syncAction.value.action !== "fetch" ? [{
                                    label: "Fetch",
                                    disabled: !canFetch.value,
                                    select: () => runGit("fetch"),
                                  }] : []),
                                ];
                                const repositoryActions = [
                                  { label: "Rename", disabled: working.value, select: () => {
                                    repoDialogError.value = "";
                                    renaming.value = { repo: currentRepo, name: currentRepo.name };
                                  } },
                                  { label: "Change root folder", disabled: working.value, select: () => {
                                    changingRoot.value = true;
                                    locating.value = currentRepo;
                                  } },
                                  { label: "Remove", disabled: working.value, select: () => openRemoveRepo(currentRepo) },
                                ];
                                const changesTab = activeTab.value === "changes";
                                const menuActions = changesTab ? [...actions, ...repositoryActions] : repositoryActions;
                                return (
                                  <>
                                    {!changesTab && actions.map(({ label, disabled, select }) => (
                                      <ShadcnButton key={label} variant="outline" disabled={disabled} onClick={select}>
                                        {label === "Fetch" && <RefreshCw size={14} />}
                                        {label}
                                      </ShadcnButton>
                                    ))}
                                    <Popover>
                                      <PopoverTrigger asChild>
                                        <ShadcnButton variant="ghost">More actions</ShadcnButton>
                                      </PopoverTrigger>
                                      <PopoverContent align="end" className="changes-actions" aria-label="More repository actions">
                                        {menuActions.map(({ label, disabled, select }) => (
                                          <PopoverClose key={label} asChild>
                                            <ShadcnButton variant="ghost" disabled={disabled} onClick={select}>
                                              {label}
                                            </ShadcnButton>
                                          </PopoverClose>
                                        ))}
                                      </PopoverContent>
                                    </Popover>
                                  </>
                                );
                              })()}
                              {syncAction.value ? (
                                  <ShadcnButton
                                    variant={activeTab.value === "changes" && currentRepo.dirty ? "outline" : "default"}
                                    disabled={!canFetch.value}
                                    onClick={() =>
                                      confirmGit(syncAction.value!.action)
                                    }
                                  >
                                    <syncAction.value.icon size={14} />{" "}
                                    {syncAction.value.label}
                                  </ShadcnButton>
                              ) : null}
                              {openMenu(currentRepo)}
                              </div>
                          </div>
                        </div>
                        <Tabs.Root
                          value={(() => activeTab.value)()}
                          onValueChange={(tab) => {
                            if (
                              tab === "changes" ||
                              tab === "history" ||
                              tab === "pulls" ||
                              tab === "activity"
                            ) {
                              activeTab.value = tab;
                              if (tab === "changes") void refresh();
                            }
                          }}
                        >
                          <TabsList
                            aria-label={"Repository views"}
                            className={cn("repository-tabs")}
                          >
                            <TabsTrigger value={"changes"} data-tour="changes">
                              <FileCode size={16} />
                              {" Changes "}
                              <span>{currentRepo.dirty}</span>
                            </TabsTrigger>
                            <TabsTrigger value={"history"} data-tour="history">
                              <History size={16} />
                              {" Commit history"}
                            </TabsTrigger>
                            {currentRepo.github_full_name ? (
                              <>
                                <TabsTrigger value={"pulls"} data-tour="pulls">
                                  <GitBranch size={16} />
                                  {" Pull requests"}
                                </TabsTrigger>
                              </>
                            ) : null}
                            <TabsTrigger value="activity">
                              <Activity size={16} />
                              Activity
                            </TabsTrigger>
                          </TabsList>
                          {launchFailures.value[currentRepo.id] ? (
                            <>
                              <div
                                role={"status"}
                                className={cn("operation-banner")}
                              >
                                <p>{launchFailures.value[currentRepo.id]}</p>
                                <Button
                                  variant={"secondary"}
                                  disabled={retryingLaunchFetch.value}
                                  onClick={() =>
                                    retryLaunchFetch(currentRepo?.id)
                                  }
                                >
                                  {"Retry fetch"}
                                </Button>
                              </div>
                            </>
                          ) : null}
                          <div className={cn("workbench-status")}>
                            <span
                              className={cn(
                                "status-label",
                                (currentRepo.dirty > 0 ||
                                  currentRepo.behind > 0 ||
                                  !!currentRepo.status_error) &&
                                  "status-warning",
                              )}
                            >
                              <i />
                              {currentRepo.status_error
                                ? "Unavailable"
                                : currentRepo.dirty
                                  ? `${currentRepo.dirty} changed`
                                  : "Clean"}
                            </span>
                            {currentRepo.ahead ? (
                              <>
                                <span>
                                  <ArrowUp size={13} />
                                  {currentRepo.ahead}
                                  {" to push"}
                                </span>
                              </>
                            ) : null}
                            {currentRepo.behind ? (
                              <>
                                <span>
                                  <ArrowDown size={13} />
                                  {currentRepo.behind}
                                  {" to pull"}
                                </span>
                              </>
                            ) : null}
                            {gitState.value?.upstream_gone ? (
                              <>
                                <span>{"Remote branch deleted"}</span>
                              </>
                            ) : null}
                            {currentRepo.ci_conclusion ||
                            currentRepo.ci_status ? (
                              <>
                                <Button className="workbench-ci" onClick={() => openRepositoryActivity(currentRepo.id)} aria-label="Open repository activity">
                                  <PlatformIcon
                                    remote={"https://github.com"}
                                    size={14}
                                  />
                                  {currentRepo.ci_conclusion ??
                                    currentRepo.ci_status}
                                </Button>
                              </>
                            ) : null}
                          </div>
                          {gitState.value?.operation ||
                          gitState.value?.conflicts.length ? (
                            <>
                              <section
                                aria-label={"Git operation"}
                                className={cn("operation-banner")}
                              >
                                <div>
                                  <strong>
                                    {gitState.value.operation
                                      ? gitState.value.operation + " paused"
                                      : "Resolve conflicting files"}
                                  </strong>
                                  <span>
                                    {gitState.value.conflicts.length
                                      ? gitState.value.conflicts.length +
                                        " conflicting files"
                                      : "Conflicts resolved"}
                                  </span>
                                </div>
                                {gitState.value.operation ? (
                                  <>
                                    <div className={cn("toolbar")}>
                                      <Button
                                        variant={"primary"}
                                        disabled={
                                          working.value ||
                                          gitState.value.conflicts.length > 0
                                        }
                                        onClick={() =>
                                          runGit("continue_operation")
                                        }
                                      >
                                        {"Continue"}
                                      </Button>
                                      <Button
                                        disabled={working.value}
                                        onClick={() =>
                                          confirmGit("abort_operation")
                                        }
                                        className={cn("secondary dangerous")}
                                      >
                                        {"Abort"}
                                      </Button>
                                    </div>
                                  </>
                                ) : null}
                              </section>
                            </>
                          ) : null}
                          {detailError.value && details.value ? (
                            <div role="status" className="workbench-status">
                              <span>Git status couldn’t refresh. Showing the last loaded changes.</span>
                              <Button
                                variant="ghost"
                                disabled={refreshing.value || working.value}
                                onClick={() => void refresh()}
                              >
                                Retry
                              </Button>
                            </div>
                          ) : null}
                          {activeTab.value === "activity" ? (
                            <Tabs.Content value="activity" className="repository-activity">
                              {currentRepo.github_full_name && (
                                accounts.value.length ? (
                                  <Button variant="secondary" disabled={working.value} onClick={sync}>
                                    <RefreshCw size={15} />
                                    Sync GitHub
                                  </Button>
                                ) : (
                                  <Button variant="secondary" onClick={openAccount}>
                                    <Github size={15} />
                                    Connect GitHub
                                  </Button>
                                )
                              )}
                              <ActivityHistory
                                items={repositoryActivity.value}
                                selectedId={activeActivityId.value}
                                onSelect={(id) => (activeActivityId.value = id)}
                              />
                            </Tabs.Content>
                          ) : detailError.value && !details.value ? (
                            <>
                              <div role={"alert"} className={cn("empty-block")}>
                                <CircleAlert size={20} />
                                <span>{detailError.value}</span>
                                <Button
                                  variant={"secondary"}
                                  onClick={() => (locating.value = currentRepo)}
                                >
                                  {"Locate folder"}
                                </Button>
                                <Button
                                  variant={"secondary"}
                                  onClick={() =>
                                    currentRepo && selectRepo(currentRepo.id)
                                  }
                                >
                                  {"Retry"}
                                </Button>
                              </div>
                            </>
                          ) : (
                            <>
                              {!details.value ? (
                                <>
                                  <div className={cn("empty-block")}>
                                    <LoaderCircle className={cn("spin")} />
                                    {" Loading Git details..."}
                                  </div>
                                </>
                              ) : (
                                <>
                                  {activeTab.value === "changes" ? (
                                    <>
                                      <Tabs.Content value={"changes"} className="changes-tab">
                                        <div
                                          className={cn("changes-workbench")}
                                        >
                                          <Card
                                            as={"section"}
                                            aria-label={"Changed files"}
                                            className={cn(
                                              "changed-files panel",
                                            )}
                                          >
                                            <div className={cn("panel-title")}>
                                              {" Changed files "}
                                              <span
                                                className={cn(
                                                  "changed-file-count",
                                                )}
                                              >
                                                {changedFiles.value.length}
                                              </span>
                                              <Button
                                                aria-label={
                                                  "Refresh changed files"
                                                }
                                                title={"Refresh changed files"}
                                                disabled={
                                                  refreshing.value ||
                                                  working.value
                                                }
                                                onClick={() => void refresh()}
                                                className={cn(
                                                  "text-action refresh-files",
                                                )}
                                              >
                                                <RefreshCw size={15} />
                                              </Button>
                                            </div>
                                            {changedFiles.value.length ? (
                                              <>
                                                <ToggleGroup
                                                  label={
                                                    "How to treat checked files"
                                                  }
                                                  value={selectionMode.value}
                                                  options={commitModes}
                                                  disabled={
                                                    working.value ||
                                                    !!gitState.value?.operation
                                                  }
                                                  onchange={(value) =>
                                                    (selectionMode.value =
                                                      value as typeof selectionMode.value)
                                                  }
                                                  className={cn(
                                                    "commit-file-mode",
                                                  )}
                                                  item={(option) => (
                                                    <>
                                                      {option.value ===
                                                      "exclude"
                                                        ? "Exclude"
                                                        : "Include"}
                                                    </>
                                                  )}
                                                />
                                              </>
                                            ) : null}
                                            {showFileSearch.value ||
                                            changedFiles.value.length > 8 ? (
                                              <>
                                                <div
                                                  className={cn(
                                                    "search-input file-search",
                                                  )}
                                                >
                                                  <Search size={13} />
                                                  <Input
                                                    ref={(node) => {
                                                      if (node)
                                                        fileSearch.value = node;
                                                    }}
                                                    aria-label={
                                                      "Filter changed files"
                                                    }
                                                    placeholder={"Filter files"}
                                                    value={fileQuery.value}
                                                    onValueChange={(next) => {
                                                      fileQuery.value = next;
                                                    }}
                                                  />
                                                </div>
                                              </>
                                            ) : null}
                                            {foldedFiles.value.length &&
                                            !fileNeedle.value ? (
                                              <>
                                                <div
                                                  className={cn("panel-title")}
                                                >
                                                  {foldedFiles.value.length}
                                                  {" grouped files "}
                                                  <Button
                                                    onClick={() =>
                                                      (showFolded.value =
                                                        !showFolded.value)
                                                    }
                                                    className={cn(
                                                      "text-action",
                                                    )}
                                                  >
                                                    {showFolded.value
                                                      ? "Collapse group"
                                                      : "Show all"}
                                                  </Button>
                                                </div>
                                              </>
                                            ) : null}
                                            <div className={cn("file-list")}>
                                              {(() => {
                                                const entries =
                                                  fileMatches.value.slice(
                                                    0,
                                                    200,
                                                  );
                                                return entries.length === 0 ? (
                                                  <>
                                                    <div
                                                      className={cn(
                                                        "empty-small",
                                                      )}
                                                    >
                                                      <Check size={17} />
                                                      {changedFiles.value.length
                                                        ? "No matching files"
                                                        : "Working tree clean"}
                                                    </div>
                                                  </>
                                                ) : (
                                                  entries.map((file, index) => (
                                                    <Fragment key={file.path}>
                                                      {(() => {
                                                        const conflicted =
                                                          gitState.value?.conflicts.includes(
                                                            file.path,
                                                          ) ?? false;
                                                        return (
                                                          <>
                                                            <div
                                                              className={cn(
                                                                "file-change-row",
                                                              )}
                                                            >
                                                              <Checkbox
                                                                aria-label={
                                                                  (selectionMode.value ===
                                                                  "include"
                                                                    ? "Include "
                                                                    : "Exclude ") +
                                                                  file.path
                                                                }
                                                                checked={markedFiles.value.has(
                                                                  file.path,
                                                                )}
                                                                disabled={
                                                                  working.value ||
                                                                  !!gitState
                                                                    .value
                                                                    ?.operation
                                                                }
                                                                onCheckedChange={(
                                                                  checked,
                                                                ) =>
                                                                  markFile(
                                                                    file.path,
                                                                    checked,
                                                                  )
                                                                }
                                                              />
                                                              <Button
                                                                aria-pressed={
                                                                  selectedFile.value ===
                                                                  file.path
                                                                }
                                                                title={
                                                                  file.path
                                                                }
                                                                onClick={() => {
                                                                  if (
                                                                    selectedFile.value !==
                                                                    file.path
                                                                  )
                                                                    void showDiff(
                                                                      file.path,
                                                                      currentRepo.id,
                                                                    );
                                                                }}
                                                                className={cn([
                                                                  "changed-file",
                                                                  {
                                                                    "file-selected":
                                                                      selectedFile.value ===
                                                                      file.path,
                                                                  },
                                                                ])}
                                                              >
                                                                <FileCode
                                                                  size={15}
                                                                />
                                                                <span>
                                                                  {file.path}
                                                                </span>
                                                                <b
                                                                  className={cn(
                                                                    (file.x ===
                                                                      "A" ||
                                                                      file.x ===
                                                                        "?") &&
                                                                      "change-added",
                                                                    (file.x ===
                                                                      "D" ||
                                                                      file.y ===
                                                                        "D") &&
                                                                      "change-deleted",
                                                                    conflicted &&
                                                                      "change-conflict",
                                                                  )}
                                                                >
                                                                  {fileStatus(
                                                                    file.x,
                                                                    file.y,
                                                                    conflicted,
                                                                  )}
                                                                </b>
                                                              </Button>
                                                              {conflicted ? (
                                                                <>
                                                                  <Button
                                                                    onClick={() =>
                                                                      (conflictFile.value =
                                                                        file.path)
                                                                    }
                                                                    className={cn(
                                                                      "resolve-file",
                                                                    )}
                                                                  >
                                                                    {"Resolve"}
                                                                  </Button>
                                                                </>
                                                              ) : null}
                                                            </div>
                                                          </>
                                                        );
                                                      })()}
                                                    </Fragment>
                                                  ))
                                                );
                                              })()}
                                            </div>
                                            {fileMatches.value.length > 200 ? (
                                              <>
                                                <small
                                                  className={cn("rail-limit")}
                                                >
                                                  {"200 of "}
                                                  {fileMatches.value.length}
                                                  {
                                                    " files. Filter to find a file."
                                                  }
                                                </small>
                                              </>
                                            ) : null}
                                          </Card>
                                          <Card
                                            as={"section"}
                                            aria-label={"File diff"}
                                            aria-busy={diffLoading.value}
                                            className={cn("diff-panel panel")}
                                          >
                                            <div className={cn("panel-title")}>
                                              <span
                                                title={selectedFile.value ?? ""}
                                                className={cn("diff-filename")}
                                              >
                                                <FileCode size={15} />
                                                {selectedFile.value ??
                                                  "Working tree"}
                                              </span>
                                              <div className="diff-header-actions">
                                                {selectedFile.value && details.value?.changes.some((change) =>
                                                  change.path === selectedFile.value && change.y !== "D" && (change.x !== "D" || change.y === "?")
                                                ) && openMenu(currentRepo, selectedFile.value)}
                                                <Button
                                                  aria-pressed={selectedFile.value === null}
                                                  disabled={selectedFile.value === null && !diffError.value}
                                                  onClick={() =>
                                                    showDiff(null, currentRepo.id)
                                                  }
                                                  className={cn("text-action")}
                                                >
                                                  {"All changes"}
                                                </Button>
                                              </div>
                                            </div>
                                            {diffLoading.value ? (
                                              <>
                                                <div
                                                  className={cn("diff-empty")}
                                                >
                                                  <LoaderCircle
                                                    size={20}
                                                    className={cn("spin")}
                                                  />
                                                  <span>
                                                    {"Loading diff..."}
                                                  </span>
                                                </div>
                                              </>
                                            ) : (
                                              <>
                                                {diffError.value && !diffCode.value ? (
                                                  <>
                                                    <div
                                                      role={"alert"}
                                                      className={cn(
                                                        "diff-empty",
                                                      )}
                                                    >
                                                      <CircleAlert size={20} />
                                                      <span>
                                                        {diffError.value}
                                                      </span>
                                                      <Button
                                                        variant={"secondary"}
                                                        onClick={() =>
                                                          showDiff(
                                                            selectedFile.value,
                                                            currentRepo.id,
                                                          )
                                                        }
                                                      >
                                                        {"Retry"}
                                                      </Button>
                                                    </div>
                                                  </>
                                                ) : (
                                                  <>
                                                    {diffCode.value ? (
                                                      <>
                                                        <DiffView
                                                          key={JSON.stringify([currentRepo.id, selectedFile.value])}
                                                          code={diffCode.value}
                                                          label={"Git diff"}
                                                        />
                                                      </>
                                                    ) : (
                                                      <>
                                                        {selectedFile.value &&
                                                        changedFiles.value.some(
                                                          (file) =>
                                                            file.path ===
                                                              selectedFile.value &&
                                                            file.x === "?",
                                                        ) ? (
                                                          <>
                                                            <div
                                                              className={cn(
                                                                "diff-empty",
                                                              )}
                                                            >
                                                              <Code2
                                                                size={25}
                                                              />
                                                              <span>
                                                                {
                                                                  "Untracked file"
                                                                }
                                                              </span>
                                                              <Button
                                                                variant={
                                                                  "secondary"
                                                                }
                                                                disabled={
                                                                  working.value
                                                                }
                                                                onClick={() =>
                                                                  selectedFile.value &&
                                                                  runGit(
                                                                    "stage_file",
                                                                    [
                                                                      selectedFile.value,
                                                                    ],
                                                                  )
                                                                }
                                                              >
                                                                {
                                                                  "Stage to preview diff"
                                                                }
                                                              </Button>
                                                            </div>
                                                          </>
                                                        ) : (
                                                          <>
                                                            <div
                                                              className={cn(
                                                                "diff-empty",
                                                              )}
                                                            >
                                                              <Code2
                                                                size={25}
                                                              />
                                                              <span>
                                                                {changedFiles
                                                                  .value.length
                                                                  ? "No textual diff. The file may be binary or unchanged."
                                                                  : "No changes"}
                                                              </span>
                                                            </div>
                                                          </>
                                                        )}
                                                      </>
                                                    )}
                                                  </>
                                                )}
                                              </>
                                            )}
                                            {diffError.value && diffCode.value ? (
                                              <div role="status" className="workbench-status">
                                                <span>Couldn’t refresh diff.</span>
                                                <Button
                                                  variant="ghost"
                                                  onClick={() => void showDiff(selectedFile.value, currentRepo.id)}
                                                >
                                                  Retry
                                                </Button>
                                              </div>
                                            ) : null}
                                          </Card>
                                        </div>
                                        <div
                                          data-tour={"commit"}
                                          className={cn(
                                            "commit-panel workbench-commit",
                                          )}
                                        >
                                          <div className={cn("commit-heading")}>
                                            {"Commit changes"}
                                          </div>
                                          {repoContext.value?.author ? (
                                            <>
                                              <p>{repoContext.value.author}</p>
                                            </>
                                          ) : (
                                            <>
                                              {repoContext.value ? (
                                                <>
                                                  <Button
                                                    variant={"secondary"}
                                                    disabled={
                                                      working.value ||
                                                      !repoMetadataReady.value ||
                                                      !workflowReady.value
                                                    }
                                                    onClick={() =>
                                                      (showRepoSettings.value = true)
                                                    }
                                                  >
                                                    {"Set Git author"}
                                                  </Button>
                                                </>
                                              ) : null}
                                            </>
                                          )}
                                          {recoveredMessage.value ? (
                                            <>
                                              <Collapsible
                                                heading={() => (
                                                  <>
                                                    {"Recovered commit message"}
                                                  </>
                                                )}
                                              >
                                                <pre>
                                                  {recoveredMessage.value}
                                                </pre>
                                                <Button
                                                  variant={"secondary"}
                                                  disabled={
                                                    recoveredMessage.value
                                                      .length > 500
                                                  }
                                                  onClick={useRecoveredMessage}
                                                >
                                                  {"Use recovered message"}
                                                </Button>
                                              </Collapsible>
                                            </>
                                          ) : null}
                                          <Textarea
                                            disabled={
                                              working.value ||
                                              draftLoading.value
                                            }
                                            aria-label={"Commit message"}
                                            placeholder={"Commit message"}
                                            value={message.value}
                                            rows={2}
                                            maxLength={500}
                                            onInput={(event) =>
                                              updateDraft(
                                                event.currentTarget.value,
                                              )
                                            }
                                          />
                                          {messageTooLong.value ? (
                                            <>
                                              <p
                                                role={"alert"}
                                                className={cn(
                                                  "connection-error",
                                                )}
                                              >
                                                {
                                                  "Shorten the commit message to 500 characters before committing."
                                                }
                                              </p>
                                            </>
                                          ) : null}
                                          {changedFiles.value.length ? (
                                            <>
                                              {(() => {
                                                const hidden =
                                                  commitFiles.value.length -
                                                  fileMatches.value
                                                    .slice(0, 200)
                                                    .filter((file) =>
                                                      commitFileSet.value.has(
                                                        file.path,
                                                      ),
                                                    ).length;
                                                return (
                                                  <>
                                                    <p aria-live={"polite"}>
                                                      {commitFiles.value.length}
                                                      {" of "}
                                                      {
                                                        changedFiles.value
                                                          .length
                                                      }
                                                      {" files"}
                                                      {hidden > 0
                                                        ? ` · ${hidden} hidden`
                                                        : ""}
                                                    </p>
                                                  </>
                                                );
                                              })()}
                                            </>
                                          ) : null}
                                          <div className={cn("toolbar inner")}>
                                            {markedFiles.value.size ? (
                                              <>
                                                <Button
                                                  variant={"secondary"}
                                                  disabled={working.value}
                                                  onClick={() =>
                                                    (markedFiles.value =
                                                      new Set())
                                                  }
                                                >
                                                  {"Reset to all files"}
                                                </Button>
                                              </>
                                            ) : null}
                                            <Button
                                              variant={"primary"}
                                              disabled={
                                                working.value ||
                                                draftLoading.value ||
                                                messageTooLong.value ||
                                                !message.value.trim() ||
                                                !!currentRepo.status_error ||
                                                !commitFiles.value.length ||
                                                !!gitState.value?.operation ||
                                                !!gitState.value?.conflicts
                                                  .length
                                              }
                                              onClick={commit}
                                            >
                                              {"Commit "}
                                              {commitFiles.value.length &&
                                              commitFiles.value.length ===
                                                changedFiles.value.length
                                                ? "all "
                                                : ""}
                                              {commitFiles.value.length}{" "}
                                              {commitFiles.value.length === 1
                                                ? "file"
                                                : "files"}
                                            </Button>
                                          </div>
                                        </div>
                                      </Tabs.Content>
                                    </>
                                  ) : (
                                    <>
                                      {activeTab.value === "history" ? (
                                        <>
                                          <Tabs.Content value={"history"}>
                                            {gitState.value?.head &&
                                            gitState.value.branch !==
                                              "(detached)" &&
                                            !gitState.value.dirty &&
                                            !gitState.value.operation &&
                                            !gitState.value.behind &&
                                            (!gitState.value.upstream ||
                                              gitState.value.ahead > 0) ? (
                                              <>
                                                <div className={cn("toolbar")}>
                                                  <Button
                                                    variant={"secondary"}
                                                    disabled={working.value}
                                                    onClick={() => {
                                                      if (
                                                        currentRepo &&
                                                        gitState.value?.head
                                                      )
                                                        undoConfirm.value = {
                                                          repo: currentRepo,
                                                          branch:
                                                            gitState.value
                                                              .branch,
                                                          head: gitState.value
                                                            .head,
                                                        };
                                                    }}
                                                  >
                                                    {"Undo last local commit…"}
                                                  </Button>
                                                </div>
                                              </>
                                            ) : null}
                                            <Fragment key={currentRepo.id}>
                                              <LoadBoundary>
                                                <Suspense fallback={null}>
                                                  <CommitHistory
                                                    repoId={currentRepo.id}
                                                    head={
                                                      gitState.value?.head ??
                                                      null
                                                    }
                                                    blocked={
                                                      working.value ||
                                                      !!gitState.value?.dirty ||
                                                      !!gitState.value
                                                        ?.operation
                                                    }
                                                    onrevert={revertCommit}
                                                  />
                                                </Suspense>
                                              </LoadBoundary>
                                            </Fragment>
                                          </Tabs.Content>
                                        </>
                                      ) : (
                                        <>
                                          {activeTab.value === "pulls" ? (
                                            <>
                                              <Tabs.Content value={"pulls"}>
                                                <Fragment
                                                  key={currentRepo.path}
                                                >
                                                  <LoadBoundary>
                                                    <Suspense fallback={null}>
                                                      <PullRequests
                                                        target={targetOf(
                                                          currentRepo,
                                                        )}
                                                        git={gitState.value}
                                                        accounts={
                                                          accounts.value
                                                        }
                                                        preferences={
                                                          activeSettings.value
                                                        }
                                                        persistentDrafts={
                                                          workflowApp.value
                                                            .save_drafts
                                                        }
                                                        onconnect={openAccount}
                                                        onchanged={() => {
                                                          void refresh();
                                                          void loadMetadata();
                                                        }}
                                                      />
                                                    </Suspense>
                                                  </LoadBoundary>
                                                </Fragment>
                                              </Tabs.Content>
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
                        </Tabs.Root>
                        <Collapsible
                          className={cn("repo-actions")}
                          heading={() => <span data-tour="repo-actions">Repository actions</span>}
                        >
                          <div className={cn("toolbar")}>
                            <Button
                              variant={"secondary"}
                              disabled={
                                !repoMetadataReady.value || !workflowReady.value
                              }
                              title="Set Git identity and repository defaults"
                              onClick={() => (showRepoSettings.value = true)}
                            >
                              {"Repository settings"}
                            </Button>
                            <Button
                              variant={"secondary"}
                              onClick={() =>
                                openLocation(currentRepo!, "editor")
                              }
                            >
                              {"Open in editor"}
                            </Button>
                            <Button
                              variant={"secondary"}
                              onClick={() =>
                                openLocation(currentRepo!, "terminal")
                              }
                            >
                              {"Open terminal"}
                            </Button>
                            <Button
                              variant={"secondary"}
                              onClick={() =>
                                openLocation(currentRepo!, "folder")
                              }
                            >
                              {"Open folder"}
                            </Button>
                            <Button
                              variant={"secondary"}
                              onClick={() => (locating.value = currentRepo)}
                            >
                              {"Locate folder"}
                            </Button>
                          </div>
                        </Collapsible>
                        {actionResult.value ? (
                          <>
                            <Card className={cn("panel")}>
                              <div className={cn("panel-title")}>
                                {"Git output"}
                              </div>
                              <pre className={cn("output")}>
                                {actionResult.value}
                              </pre>
                            </Card>
                          </>
                        ) : null}
                        {repoActivity.value.length ? (
                          <>
                            <Card className={cn("workbench-activity panel")}>
                              <div className={cn("panel-title")}>
                                {"Recent activity"}
                              </div>
                              {repoActivity.value.map((item) => (
                                <Button
                                  key={item.id}
                                  className="activity-entry"
                                  onClick={() => openRepositoryActivity(currentRepo.id, item.id)}
                                >
                                  <span className="activity-icon" aria-hidden="true">
                                    <Activity size={15} />
                                  </span>
                                  <span className="activity-text">
                                    <strong>{activityTitle(item)}</strong>
                                    {item.detail && item.kind !== "ci" && (
                                      <span>{short(item.detail, 70)}</span>
                                    )}
                                  </span>
                                  <time>{date(item.occurred_at)}</time>
                                </Button>
                              ))}
                            </Card>
                          </>
                        ) : null}
                        <Button
                          disabled={working.value}
                          onClick={() => openRemoveRepo(currentRepo)}
                          className={cn("remove-repo")}
                        >
                          <X size={13} />
                          {" Remove from workspace"}
                        </Button>
                      </section>
                    </div>
                  </>
                ) : (
                  <>
                    {view.value === "settings" ? (
                      <>
                        <div className={cn("page-heading")}>
                          <div>
                            <h1>{"Settings"}</h1>
                            <p>
                              {
                                "Manage GitHub connections and local configuration."
                              }
                            </p>
                          </div>
                        </div>
                        <Tabs.Root
                          value={settingsTab.value}
                          onValueChange={(tab) => {
                            settingsTab.value = tab === "accounts" ? "accounts" : "general";
                          }}
                        >
                          <TabsList aria-label="Settings sections">
                            <TabsTrigger value="general">General</TabsTrigger>
                            <TabsTrigger value="accounts">Accounts</TabsTrigger>
                          </TabsList>
                          <Tabs.Content value="general" className="settings-panels">
                            <Card
                              as={"section"}
                              aria-label={"Appearance"}
                              className={cn("panel workspace-settings")}
                            >
                              <h2 data-tour="appearance">Appearance</h2>
                              {appearanceReady.value ? (
                                <>
                                  <LoadBoundary>
                                    <Suspense fallback={null}>
                                      <AppearanceEditor
                                        theme={theme.value}
                                        systemDark={systemDark.value}
                                        onsave={saveAppearance}
                                      />
                                    </Suspense>
                                  </LoadBoundary>
                                </>
                              ) : (
                                <>
                                  {appearanceError.value ? (
                                    <>
                                      <Alert className={cn("error")}>
                                        {appearanceError.value}
                                      </Alert>
                                    </>
                                  ) : null}
                                  <Button
                                    variant={"secondary"}
                                    onClick={loadAppearance}
                                  >
                                    {"Load appearance"}
                                  </Button>
                                </>
                              )}
                            </Card>
                            <Card className={cn("panel workspace-settings")}>
                              <h2>{"Repository folder"}</h2>
                              <WorkspaceFolder
                                settings={workspace.value}
                                completeOnSave={workspace.value.setup_complete}
                                onsaved={updateWorkspace}
                              />
                            </Card>
                            <Card
                              className={cn(
                                "panel workspace-settings startup-settings",
                              )}
                            >
                              <h2 data-tour="startup">Startup</h2>
                              <LoadBoundary>
                                <Suspense fallback={null}>
                                  <StartupSettings
                                    repos={startupRepositories()}
                                    reposReady={repoMetadataReady.value}
                                    onfetchsaved={fetchPreferencesSaved}
                                  />
                                </Suspense>
                              </LoadBoundary>
                            </Card>
                            <Card className={cn("panel workspace-settings")}>
                              <h2>{"Workflow defaults"}</h2>
                              {workflowReady.value ? (
                                <>
                                  <LoadBoundary>
                                    <Suspense fallback={null}>
                                      <WorkflowSettings
                                        app={workflowApp.value}
                                        onsaved={(value) => {
                                          workflowApp.value =
                                            value as AppPreferences;
                                        }}
                                      />
                                    </Suspense>
                                  </LoadBoundary>
                                </>
                              ) : (
                                <>
                                  <Button
                                    variant={"secondary"}
                                    onClick={loadWorkflowApp}
                                  >
                                    {"Load workflow preferences"}
                                  </Button>
                                </>
                              )}
                            </Card>
                            <div className="settings-tools">
                              <Card className="panel walkthrough-settings">
                                <div>
                                  <strong>A quick tour</strong>
                                  <p>Revisit the basics whenever you need them.</p>
                                </div>
                                <Button variant="secondary" onClick={() => (showWalkthrough.value = true)}>
                                  Open walkthrough
                                </Button>
                              </Card>
                              <Card className="panel walkthrough-settings">
                                <div>
                                  <strong data-tour="logs">Live logs</strong>
                                  <p>View runtime activity, Git diagnostics, and error details.</p>
                                </div>
                                <Button variant="secondary" onClick={() => (showLiveLogs.value = true)}>
                                  Open live logs
                                </Button>
                              </Card>
                            </div>
                          </Tabs.Content>
                          <Tabs.Content value="accounts" className="settings-panels">
                            <AccountsSettings
                              accounts={accounts.value}
                              working={working.value}
                              onConnect={openAccount}
                              onSync={(account) => act(() => api.sync(account.id), "Synchronized " + account.login)}
                              onDisconnect={disconnectAccount}
                            />
                          </Tabs.Content>
                        </Tabs.Root>
                      </>
                    ) : (
                      <>
                        {view.value === "activity" ? (
                          <>
                            <div className={cn("page-heading")}>
                              <h1>Activity</h1>
                              <Button
                                variant={"secondary"}
                                disabled={
                                  working.value || accounts.value.length === 0
                                }
                                onClick={sync}
                              >
                                <RefreshCw size={15} />
                                {" Sync GitHub"}
                              </Button>
                            </div>
                            <ActivityHistory
                              items={activities.value}
                              selectedId={activeActivityId.value}
                              onSelect={(id) => (activeActivityId.value = id)}
                            />
                          </>
                        ) : (
                          <>
                            {(() => {
                              const latestActivity = () => (
                                <Card as="section" aria-label="Latest activity" className="latest-activity panel">
                                  <div className="latest-heading">
                                    <h2><Clock3 size={17} /> Latest activity</h2>
                                    <Button onClick={() => openWorkspaceActivity()} className="text-action">
                                      View all activity <ArrowUp size={13} />
                                    </Button>
                                  </div>
                                  <div className="latest-items">
                                    {activities.value.length === 0 ? (
                                      <div className="empty-small">No activity yet</div>
                                    ) : activities.value.slice(0, 3).map((item) => (
                                      <Button key={item.id} className="latest-item" onClick={() => openWorkspaceActivity(item.id)}>
                                        <span className="activity-icon" aria-hidden="true">
                                          {item.kind === "ci" ? <Activity size={16} /> : <GitCommitHorizontal size={16} />}
                                        </span>
                                        <span className="latest-item-content">
                                          <strong>{item.repo_name ?? "Workspace"} · {activityTitle(item)}</strong>
                                          <small>{date(item.occurred_at)}</small>
                                        </span>
                                      </Button>
                                    ))}
                                  </div>
                                </Card>
                              );
                              return (
                                <>
                                  <div className={cn("page-heading")}>
                                    <div className="collection-title">
                                      {view.value.startsWith("collection:") && <CollectionActions {...collectionActions(view.value.slice(11))} />}
                                      <h1 className={cn("overview-title")}>
                                        {view.value === "pinned"
                                          ? "Pinned repositories"
                                          : view.value === "attention"
                                            ? "Needs attention"
                                            : view.value.startsWith(
                                                  "collection:",
                                                )
                                              ? view.value.slice(11)
                                              : "Overview"}
                                      </h1>
                                    </div>
                                    <div className={cn("heading-actions")}>
                                      <Button
                                        variant={"secondary"}
                                        onClick={() => openImport()}
                                      >
                                        {"Import from…"}
                                      </Button>
                                      {view.value.startsWith("collection:") && (
                                        <Button
                                          variant="secondary"
                                          onClick={() => openOrganization(undefined, view.value.slice(11))}
                                        >
                                          <Plus size={16} />
                                          Add to collection
                                        </Button>
                                      )}
                                      <Button
                                        data-tour={"add"}
                                        variant={"primary"}
                                        disabled={working.value}
                                        onClick={openAdd}
                                      >
                                        <Plus size={16} />
                                        {" Add repository"}
                                      </Button>
                                    </div>
                                  </div>
                                  {discoveryError.value ? (
                                    <>
                                      <div
                                        role={"alert"}
                                        className={cn("discovery-banner")}
                                      >
                                        <span>{discoveryError.value}</span>
                                        <Button
                                          variant={"secondary"}
                                          onClick={() =>
                                            (view.value = "settings")
                                          }
                                        >
                                          {"Choose folder"}
                                        </Button>
                                      </div>
                                    </>
                                  ) : (
                                    <>
                                      {found.value?.items.length ? (
                                        <>
                                          <div
                                            role={"status"}
                                            className={cn("discovery-banner")}
                                          >
                                            <span>
                                              {found.value.items.length}
                                              {" new repositories found"}
                                            </span>
                                            <Button
                                              variant={"secondary"}
                                              onClick={() =>
                                                openImport(found.value)
                                              }
                                            >
                                              {"Review & import"}
                                            </Button>
                                          </div>
                                        </>
                                      ) : null}
                                    </>
                                  )}
                                  <div className={cn("shelf-tools")}>
                                    {view.value === "all" && (
                                      <ToggleGroup
                                        label="Overview view"
                                        value={overviewMode.value}
                                        options={[
                                          { value: "all", label: "All" },
                                          { value: "collections", label: "Collections", disabled: !collections.value.length },
                                        ]}
                                        onchange={(value) => {
                                          if (value === "collections" && !collections.value.length) return;
                                          overviewChoice.value = value === "collections" ? "collections" : "all";
                                          query.value = "";
                                        }}
                                        className="collection-chips overview-view-toggle"
                                      />
                                    )}
                                    <div className={cn("shelf-search-tools")}>
                                      <div
                                        className={cn(
                                          "search-input shelf-search",
                                        )}
                                      >
                                        <Search size={17} />
                                        <Input
                                          data-tour={"search"}
                                          ref={(node) => {
                                            if (node) searchInput.value = node;
                                          }}
                                          aria-label={showCollectionShelf.value ? "Search collections" : "Search repositories"}
                                          placeholder={showCollectionShelf.value ? "Search collections" : "Search repositories"}
                                          value={query.value}
                                          onValueChange={(next) => {
                                            query.value = next;
                                          }}
                                        />
                                        <kbd className={cn("key-hint")} title="Open the repository switcher">
                                          {"Ctrl K"}
                                        </kbd>
                                      </div>
                                      {!showCollectionShelf.value && (
                                      <ToggleGroup
                                        label={"Repository layout"}
                                        value={shelfLayout.value}
                                        options={shelfModes}
                                        onchange={(value) =>
                                          (shelfLayout.value =
                                            value as typeof shelfLayout.value)
                                        }
                                        className="shrink-0 flex-nowrap [&>button]:size-8 [&>button]:p-0"
                                        item={(option) => (
                                          <>
                                            {option.value === "grid" ? (
                                              <>
                                                <LayoutGrid size={16} />
                                              </>
                                            ) : (
                                              <>
                                                <List size={17} />
                                              </>
                                            )}
                                          </>
                                        )}
                                      />
                                      )}
                                    </div>
                                  </div>
                                  <div className="shelf-count">
                                    {showCollectionShelf.value
                                      ? `${collectionShelf.value.length} ${collectionShelf.value.length === 1 ? "collection" : "collections"}`
                                      : `${visible.value.length} ${visible.value.length === 1 ? "repository" : "repositories"}`}
                                  </div>
                                  {view.value === "all" && (
                                    <div className="overview-latest">{latestActivity()}</div>
                                  )}
                                  {showCollectionShelf.value ? (
                                    <section aria-label="Collections" className="overview-collections">
                                      {collectionShelf.value.map(([name, count]) => (
                                        <Card key={name} className="overview-collection-card" {...collectionDropTarget(name, moveCollection, true)}>
                                          <CollectionActions {...collectionActions(name)} />
                                          <Button
                                            className="overview-collection-open"
                                            draggable
                                            onDragStart={(event) => collectionDragStart(event, name)}
                                            onClick={() => {
                                              query.value = "";
                                              view.value = "collection:" + name;
                                            }}
                                          >
                                            <Folder size={18} />
                                            <span>
                                              <strong>{name}</strong>
                                              <small>{count} {count === 1 ? "repository" : "repositories"}</small>
                                            </span>
                                          </Button>
                                        </Card>
                                      ))}
                                      {!collectionShelf.value.length && <p className="empty-small">No collections match your search.</p>}
                                    </section>
                                  ) : (
                                    <>
                                  <div
                                    aria-label={"Repositories"}
                                    className={cn(
                                      "repository-shelf",
                                      shelfLayout.value === "rows" &&
                                        "repository-rows",
                                      shelfLayout.value === "rows" && view.value.startsWith("collection:") && "collection-rows",
                                    )}
                                  >
                                    {(() => {
                                      const entries = visible.value.slice(
                                        (page.value - 1) * pageSize,
                                        page.value * pageSize,
                                      );
                                      return entries.length === 0 ? (
                                        <>
                                          <div
                                            className={cn(
                                              "empty-large shelf-empty",
                                            )}
                                          >
                                            <div
                                              className={cn(
                                                "empty-illustration",
                                              )}
                                            >
                                              <FolderGit2
                                                size={32}
                                                strokeWidth={1.5}
                                              />
                                            </div>
                                            <h3>
                                              {loading.value
                                                ? "Opening workspace..."
                                                : repos.value.length === 0
                                                  ? "No repositories yet"
                                                  : "No repositories found"}
                                            </h3>
                                            <p>
                                              {repos.value.length === 0
                                                ? "Add an existing local Git repository."
                                                : "Try a different search or collection."}
                                            </p>
                                            <Button
                                              data-tour={"add"}
                                              variant={"primary"}
                                              disabled={working.value}
                                              onClick={openAdd}
                                            >
                                              <Plus size={16} />
                                              {" Add repository"}
                                            </Button>
                                          </div>
                                        </>
                                      ) : (
                                        entries.map((r, index) => (
                                          <Fragment key={r.id}>
                                            <Card
                                              as={"article"}
                                              className={cn("repository-card")}
                                            >
                                              {(() => {
                                                const pinButton = () => (
                                                  <>
                                                    <Button
                                                      aria-label={
                                                        r.pinned
                                                          ? "Unpin " + r.name
                                                          : "Pin " + r.name
                                                      }
                                                      title={
                                                        r.pinned
                                                          ? "Unpin"
                                                          : "Pin"
                                                      }
                                                      disabled={pendingPins.value.includes(
                                                        r.id,
                                                      )}
                                                      onClick={() =>
                                                        togglePin(r)
                                                      }
                                                      className={cn([
                                                        "card-pin",
                                                        {
                                                          "is-pinned": r.pinned,
                                                        },
                                                      ])}
                                                    >
                                                      <Pin
                                                        size={17}
                                                        fill={
                                                          r.pinned
                                                            ? "currentColor"
                                                            : "none"
                                                        }
                                                      />
                                                    </Button>
                                                  </>
                                                );
                                                const cardBranch = () => (
                                                  <>
                                                    <span
                                                      title={r.branch}
                                                      className={cn(
                                                        "card-branch",
                                                      )}
                                                    >
                                                      <GitBranch size={14} />
                                                      <span>
                                                        {r.branch || "—"}
                                                      </span>
                                                    </span>
                                                  </>
                                                );
                                                const cardStatus = () => (
                                                  <>
                                                    {r.status_pending ? (
                                                      <>
                                                        <span
                                                          className={cn("pill")}
                                                        >
                                                          {"Checking…"}
                                                        </span>
                                                      </>
                                                    ) : (
                                                      <>
                                                        {r.missing ? (
                                                          <>
                                                            <span
                                                              className={cn(
                                                                "pill red",
                                                              )}
                                                            >
                                                              {"Folder missing"}
                                                            </span>
                                                          </>
                                                        ) : (
                                                          <>
                                                            {r.status_error ? (
                                                              <>
                                                                <span
                                                                  title={
                                                                    r.status_error
                                                                  }
                                                                  className={cn(
                                                                    "pill red",
                                                                  )}
                                                                >
                                                                  {
                                                                    "Status unavailable"
                                                                  }
                                                                </span>
                                                              </>
                                                            ) : (
                                                              <>
                                                                {r.conflicts ? (
                                                                  <>
                                                                    <span
                                                                      className={cn(
                                                                        "pill red",
                                                                      )}
                                                                    >
                                                                      {
                                                                        "Conflicts to resolve"
                                                                      }
                                                                    </span>
                                                                  </>
                                                                ) : (
                                                                  <>
                                                                    {r.dirty >
                                                                    0 ? (
                                                                      <>
                                                                        <span
                                                                          className={cn(
                                                                            "pill yellow",
                                                                          )}
                                                                        >
                                                                          {
                                                                            r.dirty
                                                                          }
                                                                          {
                                                                            " changed"
                                                                          }
                                                                        </span>
                                                                      </>
                                                                    ) : (
                                                                      <>
                                                                        {r.ci_conclusion ===
                                                                        "failure" ? (
                                                                          <>
                                                                            <span
                                                                              className={cn(
                                                                                "pill red",
                                                                              )}
                                                                            >
                                                                              {
                                                                                "CI failed"
                                                                              }
                                                                            </span>
                                                                          </>
                                                                        ) : (
                                                                          <>
                                                                            {r.ahead >
                                                                              0 &&
                                                                            r.behind ===
                                                                              0 ? (
                                                                              <>
                                                                                <span
                                                                                  className={cn(
                                                                                    "pill yellow",
                                                                                  )}
                                                                                >
                                                                                  {
                                                                                    "Ready to push"
                                                                                  }
                                                                                </span>
                                                                              </>
                                                                            ) : (
                                                                              <>
                                                                                <span
                                                                                  className={cn(
                                                                                    "pill green",
                                                                                  )}
                                                                                >
                                                                                  <Check
                                                                                    size={
                                                                                      12
                                                                                    }
                                                                                  />
                                                                                  {
                                                                                    " Clean"
                                                                                  }
                                                                                </span>
                                                                              </>
                                                                            )}
                                                                          </>
                                                                        )}
                                                                      </>
                                                                    )}
                                                                  </>
                                                                )}
                                                              </>
                                                            )}
                                                          </>
                                                        )}
                                                      </>
                                                    )}
                                                  </>
                                                );
                                                const cardFooter = () => (
                                                  <>
                                                    {(() => {
                                                      const lastActivity =
                                                        activities.value.find(
                                                          (item) =>
                                                            item.repo_id ===
                                                            r.id,
                                                        );
                                                      return (
                                                        <>
                                                          <footer
                                                            className={cn(
                                                              "card-footer",
                                                            )}
                                                          >
                                                            {lastActivity ? (
                                                              <>
                                                                <span
                                                                  className={cn(
                                                                    "card-time",
                                                                  )}
                                                                >
                                                                  <Clock3
                                                                    size={12}
                                                                  />
                                                                  {date(
                                                                    lastActivity.occurred_at,
                                                                  )}
                                                                </span>
                                                              </>
                                                            ) : (
                                                              <>
                                                                <span
                                                                  className={cn(
                                                                    "card-tags",
                                                                  )}
                                                                >
                                                                  {r.tags
                                                                    .slice(0, 3)
                                                                    .join(
                                                                      " / ",
                                                                    )}
                                                                </span>
                                                              </>
                                                            )}
                                                            {r.ci_conclusion ||
                                                            r.ci_status ? (
                                                              <>
                                                                <Button className="card-ci" onClick={() => openRepositoryActivity(r.id)} aria-label={`Open activity for ${r.name}`}>
                                                                  <Activity
                                                                    size={12}
                                                                  />
                                                                  {r.ci_conclusion ??
                                                                    r.ci_status}
                                                                </Button>
                                                              </>
                                                            ) : null}
                                                          </footer>
                                                        </>
                                                      );
                                                    })()}
                                                  </>
                                                );
                                                return (
                                                  <>
                                                    {shelfLayout.value ===
                                                    "rows" ? (
                                                      <>{pinButton()}</>
                                                    ) : null}
                                                    <div
                                                      className={cn(
                                                        "card-heading",
                                                      )}
                                                    >
                                                      <Button
                                                        onClick={() =>
                                                          selectRepo(r.id)
                                                        }
                                                        className={cn(
                                                          "card-open",
                                                        )}
                                                      >
                                                        <span
                                                          className={cn(
                                                            "card-icon",
                                                          )}
                                                        >
                                                          <PlatformIcon
                                                            remote={
                                                              r.remote_url
                                                            }
                                                            size={25}
                                                          />
                                                        </span>
                                                        <h2>{r.name}</h2>
                                                      </Button>
                                                      {shelfLayout.value ===
                                                      "grid" ? (
                                                        <>{pinButton()}</>
                                                      ) : null}
                                                    </div>
                                                    <p
                                                      title={r.path}
                                                      className={cn(
                                                        "card-path",
                                                      )}
                                                    >
                                                      <span
                                                        className={cn(
                                                          "platform-name",
                                                        )}
                                                      >
                                                        {
                                                          remoteInfo(
                                                            r.remote_url,
                                                          ).label
                                                        }
                                                      </span>
                                                      {" · "}
                                                      {remoteInfo(r.remote_url)
                                                        .project ||
                                                        short(r.path, 64)}
                                                      {shelfLayout.value ===
                                                        "rows" &&
                                                      r.collection ? (
                                                        <>
                                                          {" · "}
                                                          {r.collection}
                                                        </>
                                                      ) : null}
                                                    </p>
                                                    {shelfLayout.value ===
                                                    "grid" ? (
                                                      <>
                                                        <div
                                                          className={cn(
                                                            "card-state",
                                                          )}
                                                        >
                                                          {cardBranch()}
                                                          {cardStatus()}
                                                        </div>
                                                      </>
                                                    ) : null}
                                                    <div
                                                      className={cn(
                                                        "card-counts",
                                                      )}
                                                    >
                                                      <span>
                                                        <ArrowUp size={13} />
                                                        {r.ahead}
                                                        {" ahead"}
                                                      </span>
                                                      <span>
                                                        <ArrowDown size={13} />
                                                        {r.behind}
                                                        {" behind"}
                                                      </span>
                                                      {shelfLayout.value ===
                                                        "grid" &&
                                                      r.collection ? (
                                                        <>
                                                          <span
                                                            title={r.collection}
                                                            className={cn(
                                                              "card-collection",
                                                            )}
                                                          >
                                                            {r.collection}
                                                          </span>
                                                        </>
                                                      ) : null}
                                                    </div>
                                                    {shelfLayout.value ===
                                                    "rows" ? (
                                                      <>{cardFooter()}</>
                                                    ) : null}
                                                    {shelfLayout.value ===
                                                    "rows" ? (
                                                      <>
                                                        <div
                                                          className={cn(
                                                            "card-state",
                                                          )}
                                                        >
                                                          {cardStatus()}
                                                          {cardBranch()}
                                                        </div>
                                                      </>
                                                    ) : null}
                                                    {metadata.value.get(r.id)
                                                      ?.fetched_at ? (
                                                      <>
                                                        <p
                                                          className={cn(
                                                            "card-fetch",
                                                          )}
                                                        >
                                                          {"Last fetched "}
                                                          {date(
                                                            metadata.value.get(
                                                              r.id,
                                                            )!.fetched_at!,
                                                          )}
                                                        </p>
                                                      </>
                                                    ) : null}
                                                    {r.status_error &&
                                                    r.branch ? (
                                                      <>
                                                        <p
                                                          className={cn(
                                                            "card-fetch",
                                                          )}
                                                        >
                                                          {
                                                            "Last known branch and counts"
                                                          }
                                                        </p>
                                                      </>
                                                    ) : null}
                                                    {launchFailures.value[
                                                      r.id
                                                    ] ? (
                                                      <>
                                                        <p
                                                          className={cn(
                                                            "card-fetch",
                                                          )}
                                                        >
                                                          {
                                                            launchFailures
                                                              .value[r.id]
                                                          }
                                                        </p>
                                                        <Button
                                                          variant={"secondary"}
                                                          disabled={
                                                            retryingLaunchFetch.value
                                                          }
                                                          onClick={() =>
                                                            retryLaunchFetch(
                                                              r.id,
                                                            )
                                                          }
                                                        >
                                                          {"Retry fetch"}
                                                        </Button>
                                                      </>
                                                    ) : null}
                                                    {r.missing ? (
                                                      <>
                                                        <Button
                                                          variant={"secondary"}
                                                          onClick={() =>
                                                            (locating.value = r)
                                                          }
                                                        >
                                                          {"Locate folder"}
                                                        </Button>
                                                      </>
                                                    ) : null}
                                                    {shelfLayout.value ===
                                                    "grid" ? (
                                                      <>{cardFooter()}</>
                                                    ) : null}
                                                  </>
                                                );
                                              })()}
                                            </Card>
                                          </Fragment>
                                        ))
                                      );
                                    })()}
                                  </div>
                                  {visible.value.length > pageSize ? (
                                    <>
                                      <div className={cn("pagination")}>
                                        <Button
                                          variant={"secondary"}
                                          disabled={page.value === 1}
                                          onClick={() => page.value--}
                                        >
                                          {"Previous"}
                                        </Button>
                                        <span>
                                          {"Page "}
                                          {page.value}
                                          {" of "}
                                          {Math.ceil(
                                            visible.value.length / pageSize,
                                          )}
                                        </span>
                                        <Button
                                          variant={"secondary"}
                                          disabled={
                                            page.value * pageSize >=
                                            visible.value.length
                                          }
                                          onClick={() => page.value++}
                                        >
                                          {"Next"}
                                        </Button>
                                      </div>
                                    </>
                                  ) : null}
                                    </>
                                  )}
                                  {view.value !== "all" && latestActivity()}
                                </>
                              );
                            })()}
                          </>
                        )}
                      </>
                    )}
                  </>
                )}
              </main>
            </div>
          </div>
        </AppUpdates>
        <Toaster dark={resolvedTheme.value["data-theme-mode"] === "dark"} />
        {showAdd.value ? (
          <>
            <LoadBoundary>
              <Suspense fallback={null}>
                <AddRepository
                  accounts={accounts.value}
                  repositories={repos.value}
                  defaultFolder={workspace.value.default_folder}
                  onclose={closeAdd}
                  onconnect={() => {
                    closeAdd();
                    openAccount();
                  }}
                  oncreated={repositoryAdded}
                />
              </Suspense>
            </LoadBoundary>
          </>
        ) : null}
        {showOrganize.value ? (
          <>
            <Modal
              label={
                addingCollection.value
                  ? (existingCollection.value ? "Add to collection" : "Add collection")
                  : "Organize repository"
              }
              canClose={!organizing.value}
              onclose={() => (showOrganize.value = false)}
            >
              <h2>
                {addingCollection.value
                  ? (existingCollection.value ? "Add to collection" : "Add collection")
                  : "Organize " +
                    (organizationRepo.value?.name ?? "repository")}
              </h2>
              {addingCollection.value && repos.value.length === 0 ? (
                <>
                  <p>{"Add a repository to create a collection."}</p>
                  <div className={cn("modal-actions")}>
                    <Button
                      variant={"primary"}
                      onClick={() => {
                        showOrganize.value = false;
                        openAdd();
                      }}
                    >
                      {"Add repository"}
                    </Button>
                  </div>
                </>
              ) : (
                <>
                  {addingCollection.value ? (
                    <>
                      <form
                        onSubmit={(event) => {
                          event.preventDefault();
                          void organize();
                        }}
                        className={cn("collection-form")}
                      >
                        {existingCollection.value ? (
                          <strong>{existingCollection.value}</strong>
                        ) : (
                          <>
                            <Label htmlFor="collection-name">Collection name</Label>
                            <Input
                              id="collection-name"
                              value={collectionName.value}
                              disabled={organizing.value}
                              maxLength={100}
                              required
                              onValueChange={(next) => (collectionName.value = next)}
                            />
                          </>
                        )}
                        <fieldset className="collection-repo-choices" disabled={organizing.value}>
                          <legend>Repositories ({collectionRepos.value.size} selected)</legend>
                          <Input
                            placeholder="Find repositories"
                            aria-label="Find repositories to add"
                            value={collectionQuery.value}
                            onValueChange={(next) => (collectionQuery.value = next)}
                          />
                          <div className="collection-repo-list">
                            {(() => {
                              const matches = filteredRepos(repos.value, collectionQuery.value, "all")
                                .filter((repo) => !existingCollection.value || repo.collection !== existingCollection.value);
                              return matches.length ? matches.map((repo) => (
                                <label key={repo.id} className="collection-repo-choice">
                                  <Checkbox
                                    checked={collectionRepos.value.has(repo.id)}
                                    onCheckedChange={(checked) => {
                                      const next = new Set(collectionRepos.value);
                                      if (checked) next.add(repo.id);
                                      else next.delete(repo.id);
                                      collectionRepos.value = next;
                                    }}
                                  />
                                  <span>
                                    {repo.name}
                                    {repo.collection && <small>Currently in {repo.collection}</small>}
                                  </span>
                                </label>
                              )) : <p className="empty-small">No repositories to add.</p>;
                            })()}
                          </div>
                        </fieldset>
                        {repos.value.some((repo) =>
                          collectionRepos.value.has(repo.id) && repo.collection &&
                          repo.collection !== collectionName.value.trim()
                        ) && (
                          <p className="folder-help">Selected repositories move from their current collection.</p>
                        )}
                        {organizationError.value ? (
                          <>
                            <p
                              role={"alert"}
                              className={cn("connection-error")}
                            >
                              {organizationError.value}
                            </p>
                          </>
                        ) : null}
                        <div className={cn("modal-actions")}>
                          <Button
                            variant={"secondary"}
                            disabled={organizing.value}
                            onClick={() => (showOrganize.value = false)}
                          >
                            {"Cancel"}
                          </Button>
                          <Button
                            type={"submit"}
                            variant={"primary"}
                            disabled={
                              organizing.value ||
                              collectionRepos.value.size === 0 ||
                              !collectionName.value.trim()
                            }
                          >
                            {existingCollection.value ? "Add selected" : "Save collection"}
                          </Button>
                        </div>
                      </form>
                    </>
                  ) : (
                    <>
                      <TokenInput
                        label={"Collection"}
                        items={collection.value}
                        maxItems={1}
                        maxLength={100}
                        onItemsChange={(next) => {
                          collection.value = next;
                        }}
                      />
                      <TokenInput
                        label={"Tags"}
                        items={tags.value}
                        onItemsChange={(next) => {
                          tags.value = next;
                        }}
                      />
                      <p className="folder-help">Press Enter to add a tag. Filter by it in repository search.</p>
                      {organizationError.value ? (
                        <>
                          <p role={"alert"} className={cn("connection-error")}>
                            {organizationError.value}
                          </p>
                        </>
                      ) : null}
                      <div className={cn("modal-actions")}>
                        <Button
                          variant={"secondary"}
                          disabled={organizing.value}
                          onClick={() => (showOrganize.value = false)}
                        >
                          {"Cancel"}
                        </Button>
                        <Button
                          variant={"primary"}
                          disabled={organizing.value || !organizationRepo.value}
                          onClick={organize}
                        >
                          {"Save changes"}
                        </Button>
                      </div>
                    </>
                  )}
                </>
              )}
            </Modal>
          </>
        ) : null}
        {showAccount.value ? (
          <>
            <Modal label={"Connect GitHub"} onclose={closeAccount}>
              <div className={cn("modal-top")}>
                <div>
                  <h2>{"Connect GitHub"}</h2>
                </div>
              </div>
              {loginError.value ? (
                <>
                  <p role={"alert"} className={cn("connection-error")}>
                    {loginError.value}
                  </p>
                </>
              ) : null}
              {accountMode.value === "browser" ? (
                <>
                  {loginCode.value ? (
                    <>
                      <p>{"Enter this code on GitHub:"}</p>
                      <div
                        aria-label={"GitHub sign-in code"}
                        className={cn("device-code")}
                      >
                        {loginCode.value.user_code}
                      </div>
                      <p role={"status"} className={cn("login-status")}>
                        <LoaderCircle size={15} className={cn("spin")} />
                        {loginMessage.value || "Waiting for GitHub…"}
                      </p>
                      <Button
                        onClick={reopenLogin}
                        className={cn("secondary browser-reopen")}
                      >
                        {"Open GitHub"}
                      </Button>
                    </>
                  ) : (
                    <>
                      <p>
                        {
                          "Continue in your browser to connect your account and manage public repositories and pull requests."
                        }
                      </p>
                      {browserAvailable.value === false ? (
                        <>
                          <p className={cn("connection-error")}>
                            {
                              "Browser sign-in needs app setup. You can use a token below."
                            }
                          </p>
                        </>
                      ) : null}
                      <small>
                        {
                          "GitHub access includes public repository writes and organization membership."
                        }
                      </small>
                      <Label className={cn("scope-choice")}>
                        <Checkbox
                          checked={privateRepos.value}
                          disabled={loginBusy.value}
                          onCheckedChange={(next) => {
                            privateRepos.value = next;
                          }}
                        />
                        <span>{"Include private repositories"}</span>
                      </Label>
                      {privateRepos.value ? (
                        <>
                          <small>
                            {
                              "GitHub's repository permission also grants write access. Reponiq uses this connection for repositories, workflows and pull requests."
                            }
                          </small>
                        </>
                      ) : null}
                      <div className={cn("modal-actions")}>
                        <Button variant={"secondary"} onClick={closeAccount}>
                          {"Cancel"}
                        </Button>
                        <Button
                          variant={"primary"}
                          disabled={
                            loginBusy.value ||
                            working.value ||
                            !browserAvailable.value
                          }
                          onClick={browserConnect}
                        >
                          {loginBusy.value ? (
                            <>
                              <LoaderCircle size={15} className={cn("spin")} />
                            </>
                          ) : (
                            <>
                              <Github size={16} />
                            </>
                          )}
                          {" Continue with GitHub"}
                        </Button>
                      </div>
                      <Button
                        disabled={loginBusy.value}
                        onClick={() => {
                          accountMode.value = "token";
                          loginError.value = "";
                        }}
                        className={cn("text-action auth-alternative")}
                      >
                        {"Use a personal access token"}
                      </Button>
                    </>
                  )}
                </>
              ) : (
                <>
                  <Label htmlFor={"token"}>{"Personal access token"}</Label>
                  <Input
                    id={"token"}
                    type={"password"}
                    placeholder={"github_pat_…"}
                    autoComplete={"off"}
                    value={token.value}
                    onValueChange={(next) => {
                      token.value = next;
                    }}
                  />
                  <small>
                    {
                      "For private workflows, use a fine-grained token with Actions read access."
                    }
                  </small>
                  <div className={cn("modal-actions")}>
                    <Button
                      variant={"secondary"}
                      onClick={() => {
                        accountMode.value = "browser";
                        token.value = "";
                        loginError.value = "";
                      }}
                    >
                      {"Back"}
                    </Button>
                    <Button
                      variant={"primary"}
                      disabled={working.value || !token.value.trim()}
                      onClick={connect}
                    >
                      {"Connect account"}
                    </Button>
                  </div>
                </>
              )}
            </Modal>
          </>
        ) : null}
        {showBranch.value ? (
          <>
            <Modal
              label={"Create branch"}
              onclose={() => (showBranch.value = false)}
            >
              <h2>{"Create branch"}</h2>
              <Label htmlFor={"branch-name"}>{"Branch name"}</Label>
              <Input
                id={"branch-name"}
                value={newBranch.value}
                placeholder={"feature/name"}
                onValueChange={(next) => {
                  newBranch.value = next;
                }}
                onKeyDown={(event) => {
                  if (event.key === "Enter" && !event.nativeEvent.isComposing)
                    void createBranch().then(() => {
                      if (!newBranch.value) showBranch.value = false;
                    });
                }}
              />
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  onClick={() => (showBranch.value = false)}
                >
                  {"Cancel"}
                </Button>
                <Button
                  variant={"primary"}
                  disabled={working.value || !newBranch.value.trim()}
                  onClick={() =>
                    createBranch().then(() => {
                      if (!newBranch.value) showBranch.value = false;
                    })
                  }
                >
                  {"Create & switch"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
        {conflictFile.value && currentRepo ? (
          <>
            <Fragment key={currentRepo.id + ":" + conflictFile.value}>
              <LoadBoundary>
                <Suspense fallback={null}>
                  <ConflictEditor
                    repoId={currentRepo.id}
                    file={conflictFile.value}
                    expected={
                      gitState.value
                        ? {
                            branch: gitState.value.branch,
                            head: gitState.value.head,
                          }
                        : null
                    }
                    onclose={() => (conflictFile.value = null)}
                    onsaved={() => void refresh()}
                  />
                </Suspense>
              </LoadBoundary>
            </Fragment>
          </>
        ) : null}
        {pendingGit.value ? (
          <>
            <Modal
              label={pendingGit.value.title}
              onclose={() => (pendingGit.value = null)}
            >
              <h2>{pendingGit.value.title}</h2>
              <p className={cn("confirm-description")}>
                {pendingGit.value.description}
              </p>
              <p className={cn("confirm-context")}>
                {
                  repos.value.find((repo) => repo.id === pendingGit.value!.id)
                    ?.name
                }
                {" · "}
                {pendingGit.value.expected.branch}
              </p>
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  onClick={() => (pendingGit.value = null)}
                >
                  {"Cancel"}
                </Button>
                <Button variant={"primary"} onClick={executePendingGit}>
                  {"Confirm"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
        {showWalkthrough.value ? (
          <>
            <Walkthrough
              prepare={prepareTour}
              onclose={() => (showWalkthrough.value = false)}
            />
          </>
        ) : null}
        {showLiveLogs.value ? (
          <>
            <LoadBoundary>
              <Suspense fallback={null}>
                <LiveLogs onclose={() => (showLiveLogs.value = false)} />
              </Suspense>
            </LoadBoundary>
          </>
        ) : null}
        {showBranches.value && details.value ? (
          <>
            <LoadBoundary>
              <Suspense fallback={null}>
                <Branches
                  details={details.value}
                  error={error.value}
                  recentBranches={recentBranches.value}
                  defaultBranch={repoContext.value?.default_branch ?? null}
                  busy={working.value}
                  onclose={() => (showBranches.value = false)}
                  onswitch={(branch, strategy) =>
                    runGit("switch_branch", [branch.reference, strategy])
                  }
                  onfetch={() => runGit("fetch")}
                  oncreate={() => {
                    showBranches.value = false;
                    newBranch.value = "";
                    showBranch.value = true;
                  }}
                  onopenworktree={(reference) => {
                    if (currentRepo)
                      void openLocation(currentRepo, "worktree", reference);
                  }}
                />
              </Suspense>
            </LoadBoundary>
          </>
        ) : null}
        {showSavedChanges.value && details.value ? (
          <>
            <Modal
              label={"Saved changes"}
              canClose={!working.value}
              onclose={() => (showSavedChanges.value = false)}
            >
              <h2>Saved changes</h2>
              <p className="folder-help">Apply a stash to bring its changes into the current branch.</p>
              <p>
                {details.value.git.branch}
                {details.value.git.dirty ? (
                  <>{" · Working files changed"}</>
                ) : null}
              </p>
              {error.value ? (
                <>
                  <p role={"alert"} className={cn("connection-error")}>
                    {error.value}
                  </p>
                </>
              ) : null}
              <div className={cn("branch-results")}>
                {(() => {
                  const entries = details.value.saved_changes;
                  return entries.map((entry, index) => (
                    <Fragment key={entry.oid}>
                      <div className={cn("quick-repo-row")}>
                        <span>
                          {index === 0 ? "Latest · " : ""}
                          {new Date(entry.timestamp * 1000).toLocaleString()}
                          {" · "}
                          {entry.oid.slice(0, 7)}
                        </span>
                        <Button
                          variant={"secondary"}
                          aria-label={
                            "Restore saved changes " + entry.oid.slice(0, 7)
                          }
                          disabled={!canRestore.value}
                          onClick={async () => {
                            if (await runGit("restore_changes", [entry.oid]))
                              showSavedChanges.value = false;
                          }}
                        >
                          {"Restore"}
                        </Button>
                      </div>
                    </Fragment>
                  ));
                })()}
              </div>
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  disabled={working.value}
                  onClick={() => (showSavedChanges.value = false)}
                >
                  {"Close"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
        {showQuick.value ? (
          <>
            <QuickSwitch
              repos={orderRepos(repos.value)}
              metadata={metadata.value}
              canOpen={!working.value}
              onselect={(repo) => {
                showQuick.value = false;
                void selectRepo(repo.id);
              }}
              onclose={() => (showQuick.value = false)}
              onmove={arrangePin}
              onlocation={(repo, action) => {
                showQuick.value = false;
                void openLocation(repo, action);
              }}
            />
          </>
        ) : null}
        {showRepoSettings.value && currentRepo ? (
          <>
            <Modal
              label={"Repository settings"}
              onclose={() => (showRepoSettings.value = false)}
            >
              <h2>
                {currentRepo.name}
                {" settings"}
              </h2>
              <Fragment key={currentRepo.path}>
                <LoadBoundary>
                  <Suspense fallback={null}>
                    <WorkflowSettings
                      target={targetOf(currentRepo)}
                      app={workflowApp.value}
                      preferences={activeSettings.value ?? null}
                      context={repoContext.value}
                      onsaved={(value) => {
                        if (currentRepo) {
                          const next = new Map(metadata.value);
                          next.set(currentRepo.id, {
                            id: currentRepo.id,
                            path: currentRepo.path,
                            pr_login: null,
                            opened_at: null,
                            pin_position: null,
                            fetched_at: null,
                            main_worktree: null,
                            cached_status: null,
                            ...activeSettings.value,
                            ...(value as RepoPreferences),
                          });
                          metadata.value = next;
                        }
                        void loadMetadata();
                      }}
                      onidentity={(value) => (repoContext.value = value)}
                    />
                  </Suspense>
                </LoadBoundary>
              </Fragment>
            </Modal>
          </>
        ) : null}
        {collectionEdit.value && (
          <Modal
            label={collectionEdit.value.kind === "rename" ? "Rename collection" : "Delete collection"}
            canClose={!collectionPending.value}
            onclose={() => (collectionEdit.value = null)}
          >
            <h2>{collectionEdit.value.kind === "rename" ? "Rename collection" : `Delete ${collectionEdit.value.current}?`}</h2>
            <form onSubmit={(event) => { event.preventDefault(); void saveCollection(); }}>
              {collectionEdit.value.kind === "rename" ? (
                <>
                  <Label htmlFor="collection-new-name">Name</Label>
                  <Input
                    id="collection-new-name"
                    autoFocus
                    disabled={collectionPending.value}
                    value={collectionEdit.value.name}
                    onValueChange={(name) => {
                      if (collectionEdit.value) collectionEdit.value = { ...collectionEdit.value, name };
                    }}
                  />
                </>
              ) : <p>Repositories will stay in Reponiq and move out of this collection.</p>}
              {collectionError.value && <p role="alert" className="connection-error">{collectionError.value}</p>}
              <div className="modal-actions">
                <Button type="button" variant="secondary" disabled={collectionPending.value} onClick={() => (collectionEdit.value = null)}>Cancel</Button>
                <ShadcnButton
                  type="submit"
                  variant={collectionEdit.value.kind === "delete" ? "destructive" : "default"}
                  disabled={collectionPending.value || !collectionEdit.value.name.trim()}
                >
                  {collectionEdit.value.kind === "rename" ? "Rename" : "Delete collection"}
                </ShadcnButton>
              </div>
            </form>
          </Modal>
        )}
        {renaming.value && (
          <Modal label="Rename repository" canClose={!working.value} onclose={() => (renaming.value = null)}>
            <h2>Rename repository</h2>
            <form onSubmit={(event) => { event.preventDefault(); void manageRepository("rename"); }}>
              <Label htmlFor="repository-display-name">Name</Label>
              <Input
                id="repository-display-name"
                autoFocus
                disabled={working.value}
                value={renaming.value.name}
                onValueChange={(name) => {
                  if (renaming.value) renaming.value = { ...renaming.value, name };
                }}
              />
              {repoDialogError.value && <p role="alert" className="connection-error">{repoDialogError.value}</p>}
              <div className="modal-actions">
                <Button variant="secondary" type="button" disabled={working.value} onClick={() => (renaming.value = null)}>Cancel</Button>
                <Button variant="primary" type="submit" disabled={working.value || !renaming.value.name.trim()}>Rename</Button>
              </div>
            </form>
          </Modal>
        )}
        {removing.value && (
          <Modal label="Remove repository" canClose={!working.value} onclose={() => (removing.value = null)}>
            <h2>Remove {removing.value.repo.name}?</h2>
            <p>{readablePath(removing.value.repo.path)}</p>
            <Label className="scope-choice" htmlFor="delete-repository-folder">
              <Switch
                id="delete-repository-folder"
                checked={removing.value.deleteFolder}
                disabled={working.value}
                onCheckedChange={(deleteFolder) => {
                  if (removing.value) removing.value = { ...removing.value, deleteFolder };
                }}
              />
              Delete the full folder
            </Label>
            <p>{removing.value.deleteFolder ? "This permanently deletes the Git repository and all files in the folder." : "The folder and its files will stay on this device."}</p>
            {repoDialogError.value && <p role="alert" className="connection-error">{repoDialogError.value}</p>}
            <div className="modal-actions">
              <Button variant="secondary" disabled={working.value} onClick={() => (removing.value = null)}>Cancel</Button>
              <ShadcnButton variant="destructive" disabled={working.value} onClick={() => void manageRepository("remove")}>
                {removing.value.deleteFolder ? "Delete folder and remove" : "Remove from Reponiq"}
              </ShadcnButton>
            </div>
          </Modal>
        )}
        {locating.value ? (
          <>
            <LocateRepo
              repo={locating.value}
              changeRoot={changingRoot.value}
              mainWorktree={
                metadata.value.get(locating.value.id)?.main_worktree
              }
              onclose={() => { locating.value = null; changingRoot.value = false; }}
              onlocated={async () => {
                const previous = locating.value;
                if (!previous) return;
                locating.value = null;
                changingRoot.value = false;
                statusVersions.set(
                  previous.id,
                  (statusVersions.get(previous.id) ?? 0) + 1,
                );
                await loadFlight;
                await load();
                await loadMetadata();
                if (selected.value === previous.id)
                  await selectRepo(previous.id);
              }}
              onadded={async (repo) => {
                locating.value = null;
                changingRoot.value = false;
                await repositoryAdded(repo);
              }}
            />
          </>
        ) : null}
        {defaultCommit.value ? (
          <>
            <Modal
              label={"Commit on the default branch"}
              onclose={() => (defaultCommit.value = null)}
            >
              <h2>
                {"Commit on "}
                {defaultCommit.value.branch}
                {"?"}
              </h2>
              <p>
                {"Create a branch to keep this work separate, or commit "}
                {defaultCommit.value.files.length}{" "}
                {defaultCommit.value.files.length === 1 ? "file" : "files"}
                {" directly to "}
                {defaultCommit.value.branch}
                {"."}
              </p>
              <Label>
                {"New branch name"}
                <Input
                  value={branchNameForCommit.value}
                  placeholder={"feature/my-change"}
                  onValueChange={(next) => {
                    branchNameForCommit.value = next;
                  }}
                />
              </Label>
              <div className={cn("modal-actions")}>
                <Button
                  onClick={() => (defaultCommit.value = null)}
                  className={cn("text-action")}
                >
                  {"Cancel"}
                </Button>
                <Button
                  variant={"secondary"}
                  onClick={() => acceptDefaultCommit(false)}
                >
                  {"Commit here"}
                </Button>
                <Button
                  variant={"primary"}
                  disabled={!branchNameForCommit.value.trim()}
                  onClick={() => acceptDefaultCommit(true)}
                >
                  {"Create branch with my edits"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
        {undoConfirm.value ? (
          <>
            <Modal
              label={"Undo last commit"}
              onclose={() => (undoConfirm.value = null)}
            >
              <h2>{"Undo the last commit?"}</h2>
              <p>
                {"Your changes will stay staged on "}
                {undoConfirm.value.branch}
                {
                  ". Reponiq checks whether the commit is already shared before undoing it."
                }
              </p>
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  onClick={() => (undoConfirm.value = null)}
                >
                  {"Cancel"}
                </Button>
                <Button variant={"primary"} onClick={undoCommit}>
                  {"Undo commit and keep changes"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
        {showSetup.value &&
        !showImport.value &&
        !showAccount.value &&
        setupStep.value !== "complete" ? (
          <>
            <Modal
              label={"Set up Reponiq"}
              canClose={false}
              showCloseButton={false}
              onclose={() => {}}
            >
              <div className={cn("setup-wizard")}>
                <div className="setup-progress">
                  <span aria-hidden="true">
                    {setupSteps.indexOf(setupStep.value) + 1}/{setupSteps.length}
                  </span>
                  <Progress
                    label="Setup progress"
                    max={setupSteps.length}
                    value={setupSteps.indexOf(setupStep.value) + 1}
                  />
                </div>
                {setupStep.value === "appearance" ? (
                  <>
                    <h2>{"Choose your appearance"}</h2>
                    <LoadBoundary>
                      <Suspense fallback={null}>
                        <AppearanceEditor
                          theme={theme.value}
                          systemDark={systemDark.value}
                          busy={advancingSetup.value}
                          onsave={saveAppearance}
                          oncontinue={() => {
                            void continueSetup("appearance").catch(() => {});
                          }}
                        />
                        {setupError.value && (
                          <Alert className="error">{setupError.value}</Alert>
                        )}
                      </Suspense>
                    </LoadBoundary>
                  </>
                ) : (
                  <>
                    {setupStep.value === "workspace" ? (
                      <>
                        <h2>{"Set up your workspace"}</h2>
                        <WorkspaceFolder
                          settings={workspace.value}
                          completeOnSave={false}
                          busy={setupBusy.value}
                          dirty={setupFolderDirty.value}
                          onsaved={updateWorkspace}
                          onBusyChange={(next) => {
                            setupBusy.value = next;
                          }}
                          onDirtyChange={(next) => {
                            setupFolderDirty.value = next;
                          }}
                        />
                        <div className={cn("setup-actions")}>
                          <Button
                            variant={"secondary"}
                            disabled={
                              setupBusy.value ||
                              setupFolderDirty.value ||
                              identityBusy.value
                            }
                            onClick={() => openImport(found.value)}
                          >
                            {"Import repositories"}
                            {found.value?.items.length
                              ? " (" + found.value.items.length + ")"
                              : ""}
                          </Button>
                        </div>
                        {discoveryError.value ? (
                          <>
                            <Alert className={cn("error")}>
                              {discoveryError.value}
                            </Alert>
                          </>
                        ) : null}
                        {repos.value.length ? (
                          <>
                            <Collapsible
                              className={cn("setup-identity-options")}
                              heading={() => (
                                <>{"Set Git identity (optional)"}</>
                              )}
                            >
                              <LoadBoundary>
                                <Suspense fallback={null}>
                                  <SetupIdentity
                                    repos={repos.value}
                                    busy={identityBusy.value}
                                    onBusyChange={(next) => {
                                      identityBusy.value = next;
                                    }}
                                  />
                                </Suspense>
                              </LoadBoundary>
                            </Collapsible>
                          </>
                        ) : null}
                        {setupError.value ? (
                          <>
                            <Alert className={cn("error")}>
                              {setupError.value}
                            </Alert>
                          </>
                        ) : null}
                        <div className={cn("modal-actions")}>
                          <Button
                            variant={"primary"}
                            disabled={
                              setupBusy.value ||
                              setupFolderDirty.value ||
                              identityBusy.value ||
                              advancingSetup.value
                            }
                            onClick={() =>
                              continueSetup("workspace").catch(() => {})
                            }
                          >
                            {workspace.value.default_folder ||
                            repos.value.length
                              ? "Continue"
                              : "Start empty"}
                          </Button>
                        </div>
                      </>
                    ) : (
                      <>
                        {setupStep.value === "accounts" ? (
                          <>
                            <h2>Connect GitHub</h2>
                            <p>Connect an account to view pull requests and workflow runs.</p>
                            {accounts.value.map((account) => (
                              <div className="account-row" key={account.id}>
                                <PlatformIcon remote="https://github.com" size={19} />
                                <span>@{account.login}</span>
                                <Badge variant="secondary">Connected</Badge>
                              </div>
                            ))}
                            <Button variant="secondary" onClick={openAccount} disabled={advancingSetup.value}>
                              {accounts.value.length ? "Connect another account" : "Connect GitHub"}
                            </Button>
                            {setupError.value && <Alert className="error">{setupError.value}</Alert>}
                            <div className="modal-actions">
                              <Button
                                variant="primary"
                                disabled={advancingSetup.value}
                                onClick={() => void continueSetup("accounts").catch(() => {})}
                              >
                                {accounts.value.length ? "Continue" : "Continue without GitHub"}
                              </Button>
                            </div>
                          </>
                        ) : setupStep.value === "startup" ? (
                          <>
                            <h2>{"Startup preferences"}</h2>
                            <LoadBoundary>
                              <Suspense fallback={null}>
                                <StartupSettings
                                  initial={true}
                                  repos={startupRepositories()}
                                  reposReady={repoMetadataReady.value}
                                  onfetchsaved={fetchPreferencesSaved}
                                  busy={startupBusy.value}
                                  ondrafts={(value) =>
                                    (workflowApp.value = value)
                                  }
                                  ondone={() => continueSetup("startup")}
                                  onBusyChange={(next) => {
                                    startupBusy.value = next;
                                  }}
                                />
                              </Suspense>
                            </LoadBoundary>
                          </>
                        ) : (
                          <>
                            <h2>{"Ready to use Reponiq"}</h2>
                            <p>{"You can adjust these settings any time."}</p>
                            {setupError.value ? (
                              <>
                                <Alert className={cn("error")}>
                                  {setupError.value}
                                </Alert>
                              </>
                            ) : null}
                            <div className={cn("modal-actions")}>
                              <Button
                                variant={"primary"}
                                disabled={advancingSetup.value}
                                onClick={() =>
                                  continueSetup("review").catch(() => {})
                                }
                              >
                                {"Open workspace"}
                              </Button>
                            </div>
                          </>
                        )}
                      </>
                    )}
                  </>
                )}
              </div>
            </Modal>
          </>
        ) : null}
        {showGuideChoice.value ? (
          <>
            <Modal
              label={"Quick guide"}
              canClose={!guideBusy.value}
              onclose={() => chooseGuide(false)}
            >
              <h2>{"Take a quick look around?"}</h2>
              <p>
                {"Learn the daily Git workflow, from opening a repository to publishing a branch and reviewing a pull request."}
              </p>
              {guideError.value ? (
                <>
                  <Alert className={cn("error")}>{guideError.value}</Alert>
                </>
              ) : null}
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  disabled={guideBusy.value}
                  onClick={() => chooseGuide(false)}
                >
                  {"Skip guide"}
                </Button>
                <Button
                  variant={"primary"}
                  disabled={guideBusy.value}
                  onClick={() => chooseGuide(true)}
                >
                  {"Start guide"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
        {showImport.value ? (
          <>
            <LoadBoundary>
              <Suspense fallback={null}>
                <ImportRepositories
                  initial={importInitial.value}
                  defaultFolder={workspace.value.default_folder}
                  onclose={() => {
                    showImport.value = false;
                    void load();
                  }}
                  onimported={imported}
                />
              </Suspense>
            </LoadBoundary>
          </>
        ) : null}
        <ConfirmDialog />
      </>
    );
  };
}
export default observer(function App(props: Props) {
  return useController(createView, props);
});
