import {
ArrowLeft,
Check,
CircleCheck,
ChevronDown,
Download,
FolderOpen,
Globe,
LoaderCircle,
Lock,
Plus,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import {
api,
readablePath,
type Account,
type OnlineRepo,
type Repo,
type RepositoryOwner,
} from "../../lib/api";
import { cloneFolderName } from "../../lib/clone";
import { loadOnlineCatalog } from "../../lib/online-catalog";
import {
SelectOption,
derived,
state,
useController,
type Lifecycle
} from "../controller";
import { Button,Input,Label,NativeSelect,Tabs,TabsList,TabsTrigger } from "../ui";
import { Button as ShadcnButton } from "../shadcn/button";
import { cn } from "../utils";
import Modal from "./Modal";
import PlatformIcon from "./PlatformIcon";
type Props = {
  accounts: Account[];
  repositories: Repo[];
  defaultFolder?: string | null;
  onclose: () => void;
  onconnect: () => void;
  oncreated: (repo: Repo, warning?: string | null) => Promise<void>;
};
function createView(lifecycle: Lifecycle<Props>) {
  const accounts = derived(() => lifecycle.props().accounts);
  const defaultFolder = derived(() => lifecycle.props().defaultFolder ?? null);
  const onclose = derived(() => lifecycle.props().onclose);
  const onconnect = derived(() => lifecycle.props().onconnect);
  const oncreated = derived(() => lifecycle.props().oncreated);
  let mode = state<"choose" | "folder" | "clone" | "create">("choose"),
    busy = state(false),
    error = state("");
  let folder = state(""),
    parent = state(""),
    name = state(""),
    accountId = state(0),
    owner = state(""),
    isPrivate = state(true);
  let owners = state<RepositoryOwner[]>([]),
    ownerPage = state(1),
    moreOwners = state(false),
    ownerWarning = state(""),
    ownersLoading = state(false);
  let expanded = state<number | null>(null),
    rows = state<OnlineRepo[]>([]),
    page = state(1),
    hasMore = state(false),
    listLoading = state(false),
    listError = state(""),
    query = state("");
  const catalog = state<OnlineRepo[] | null>(null);
  const catalogLoading = state(false);
  const catalogError = state("");
  const searchPage = state(1);
  const searching = derived(() => query.value.trim().length > 0);
  const browseSize = 25;
  let selected = state<{
    accountId: number;
    repo: OnlineRepo;
  } | null>(null);
  let cloneSource = state<"account" | "url">("account"),
    cloneUrl = state(""),
    cloning = state(false);
  let sequence = 0,
    catalogSequence = 0,
    ownerSequence = 0,
    alive = true;
  const sorted = derived(() =>
    accounts.value.slice().sort((a, b) => a.login.localeCompare(b.login)),
  );
  const added = derived(() => new Set(
    lifecycle.props().repositories.flatMap((repo) =>
      repo.github_full_name ? [repo.github_full_name.toLowerCase()] : [],
    ),
  ));
  function alreadyAdded(repo: OnlineRepo) {
    return added.value.has(repo.full_name.toLowerCase());
  }
  const filtered = derived(() => {
    const search = query.value.trim().toLowerCase();
    return (search ? catalog.value ?? [] : rows.value)
      .filter((repo) => repo.full_name.toLowerCase().includes(search))
      .sort((a, b) => Number(alreadyAdded(a)) - Number(alreadyAdded(b)));
  });
  const displayed = derived(() => searching.value
    ? filtered.value.slice((searchPage.value - 1) * browseSize, searchPage.value * browseSize)
    : filtered.value);
  lifecycle.mount(() => {
    parent.value = readablePath(defaultFolder.value ?? "");
  });
  lifecycle.destroy(() => {
    alive = false;
    ++sequence;
    ++ownerSequence;
    ++catalogSequence;
  });
  function choose(next: typeof mode.value) {
    mode.value = next;
    error.value = "";
    if (next === "create") {
      accountId.value = sorted.value[0]?.id ?? 0;
      void loadOwners();
    }
    if (next === "clone") {
      cloneSource.value = sorted.value.length ? "account" : "url";
      name.value =
        cloneSource.value === "url" ? cloneFolderName(cloneUrl.value) : "";
      if (sorted.value[0]) void expand(sorted.value[0].id);
    }
  }
  function changeCloneSource(source: typeof cloneSource.value) {
    if (source === "url") changeCatalogQuery("");
    cloneSource.value = source;
    error.value = "";
    name.value =
      source === "url"
        ? cloneFolderName(cloneUrl.value)
        : (selected.value?.repo.name ?? "");
  }
  function changeCloneUrl(value: string) {
    if (!name.value || name.value === cloneFolderName(cloneUrl.value))
      name.value = cloneFolderName(value);
    cloneUrl.value = value;
    error.value = "";
  }
  async function browse(field: "folder" | "parent") {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      const path = await api.pickRepoFolder(
        field === "folder" ? "repository" : "parent",
      );
      if (alive && path !== null) {
        if (field === "folder") folder.value = path;
        else parent.value = path;
      }
    } catch (e) {
      if (alive) error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  async function expand(id: number, next = 1) {
    if (expanded.value !== id) catalog.value = null;
    changeCatalogQuery("");
    expanded.value = id;
    page.value = next;
    query.value = "";
    rows.value = [];
    listError.value = "";
    listLoading.value = true;
    selected.value = null;
    const request = ++sequence;
    try {
      const result = await api.onlineRepos(id, next);
      if (alive && request === sequence) {
        rows.value = result.items;
        hasMore.value = result.has_more;
      }
    } catch (e) {
      if (alive && request === sequence) listError.value = String(e);
    } finally {
      if (alive && request === sequence) listLoading.value = false;
    }
  }
  function changeCatalogQuery(value: string) {
    query.value = value;
    searchPage.value = 1;
    if (!searching.value) {
      ++catalogSequence;
      catalogLoading.value = false;
      catalogError.value = "";
    } else if (!catalog.value && !catalogLoading.value && !catalogError.value && expanded.value !== null) {
      void searchAccount(expanded.value);
    }
  }
  async function searchAccount(id: number) {
    const request = ++catalogSequence;
    const isCurrent = () => alive && request === catalogSequence;
    catalogError.value = "";
    catalogLoading.value = true;
    try {
      const result = await loadOnlineCatalog(
        (page) => api.onlineRepos(id, page, true),
        isCurrent,
      );
      if (isCurrent()) catalog.value = result;
    } catch (reason) {
      if (isCurrent()) catalogError.value = String(reason);
    } finally {
      if (isCurrent()) catalogLoading.value = false;
    }
  }
  async function loadOwners(next = 1) {
    const request = ++ownerSequence;
    const account = accountId.value;
    ownersLoading.value = account !== 0;
    ownerWarning.value = "";
    if (next === 1) {
      owners.value = [];
      owner.value = "";
    }
    if (!account) return;
    try {
      const result = await api.repositoryOwners(account, next);
      if (alive && request === ownerSequence) {
        owners.value =
          next === 1
            ? result.items
            : [
                ...owners.value,
                ...result.items.filter(
                  (item) => !owners.value.some((o) => o.login === item.login),
                ),
              ];
        owner.value = owner.value || owners.value[0]?.login || "";
        ownerPage.value = next;
        moreOwners.value = result.has_more;
        ownerWarning.value = result.warning ?? "";
      }
    } catch (e) {
      if (alive && request === ownerSequence) ownerWarning.value = String(e);
    } finally {
      if (alive && request === ownerSequence) ownersLoading.value = false;
    }
  }
  async function submit() {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      if (mode.value === "folder") {
        await oncreated.value(await api.add(folder.value.trim()));
      } else if (mode.value === "clone") {
        cloning.value = true;
        if (cloneSource.value === "url") {
          await oncreated.value(
            await api.cloneRepositoryUrl(
              cloneUrl.value.trim(),
              parent.value.trim(),
              name.value.trim(),
            ),
          );
        } else if (selected.value) {
          await oncreated.value(
            await api.cloneRepository(
              selected.value.accountId,
              selected.value.repo.id,
              parent.value.trim(),
              name.value.trim(),
            ),
          );
        }
      } else if (mode.value === "create") {
        const result = await api.createRepository({
          parent: parent.value.trim(),
          name: name.value.trim(),
          account_id: accountId.value || null,
          owner: accountId.value ? owner.value : null,
          private: isPrivate.value,
        });
        await oncreated.value(result.repo, result.warning);
      }
    } catch (e) {
      if (alive) error.value = String(e);
    } finally {
      busy.value = false;
      cloning.value = false;
    }
  }
  return () => (
    <>
      <Modal
        label={
          mode.value === "choose"
            ? "Add repository"
            : mode.value === "clone"
              ? "Clone repository"
              : mode.value === "create"
                ? "Create repository"
                : "Add from folder"
        }
        canClose={!busy.value}
        onclose={onclose.value}
      >
        <div className={cn("modal-top")}>
          <div>
            {mode.value !== "choose" ? (
              <>
                <Button
                  disabled={busy.value}
                  onClick={() => {
                    changeCatalogQuery("");
                    mode.value = "choose";
                    error.value = "";
                    ++sequence;
                    ++ownerSequence;
                  }}
                  className={cn("text-action add-back")}
                >
                  <ArrowLeft size={14} />
                  {" Back"}
                </Button>
              </>
            ) : null}
            <h2>
              {mode.value === "choose"
                ? "Add repository"
                : mode.value === "clone"
                  ? "Clone"
                  : mode.value === "create"
                    ? "Create repository"
                    : "Add from folder"}
            </h2>
          </div>
        </div>
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
        {mode.value === "choose" ? (
          <>
            <div className="grid gap-1">
              {([
                { mode: "create", icon: Plus, label: "Create", description: "Start a new repository." },
                { mode: "folder", icon: FolderOpen, label: "Add from folder", description: "Open an existing local repository." },
                { mode: "clone", icon: Download, label: "Clone", description: "Download a repository from online." },
              ] as const).map(({ mode, icon: Icon, label, description }) => (
                <ShadcnButton
                  key={mode}
                  variant="ghost"
                  className="add-repository-choice h-auto w-full justify-start gap-3 whitespace-normal px-3 py-3 text-left"
                  onClick={() => choose(mode)}
                >
                  <Icon aria-hidden="true" className="text-primary" />
                  <span className="grid min-w-0">
                    <span>{label}</span>
                    <small className="text-xs font-normal text-muted-foreground">{description}</small>
                  </span>
                </ShadcnButton>
              ))}
            </div>
          </>
        ) : (
          <>
            {mode.value === "folder" ? (
              <>
                <Label htmlFor={"repo-folder"}>{"Repository folder"}</Label>
                <div className={cn("folder-picker")}>
                  <Input
                    id={"repo-folder"}
                    value={folder.value}
                    placeholder={"Choose a repository folder"}
                    disabled={busy.value}
                    onValueChange={(next) => {
                      folder.value = next;
                    }}
                  />
                  <Button
                    variant={"secondary"}
                    disabled={busy.value}
                    onClick={() => browse("folder")}
                  >
                    <FolderOpen size={16} />
                    {" Browse…"}
                  </Button>
                </div>
                <div className={cn("modal-actions")}>
                  <Button
                    variant={"secondary"}
                    disabled={busy.value}
                    onClick={onclose.value}
                  >
                    {"Cancel"}
                  </Button>
                  <Button
                    variant={"primary"}
                    disabled={busy.value || !folder.value.trim()}
                    onClick={submit}
                  >
                    {busy.value ? (
                      <>
                        <LoaderCircle size={15} className={cn("spin")} />
                      </>
                    ) : null}
                    {" Add repository"}
                  </Button>
                </div>
              </>
            ) : (
              <>
                {mode.value === "clone" ? (
                  <>
                    <Tabs.Root
                      value={(() => cloneSource.value)()}
                      onValueChange={(source) => {
                        if (source === "account" || source === "url")
                          changeCloneSource(source);
                      }}
                    >
                      <TabsList
                        aria-label={"Clone source"}
                        className={cn("clone-sources")}
                      >
                        <TabsTrigger value={"account"} disabled={busy.value}>
                          {"GitHub accounts"}
                        </TabsTrigger>
                        <TabsTrigger value={"url"} disabled={busy.value}>
                          {"Repository URL"}
                        </TabsTrigger>
                      </TabsList>
                      {cloneSource.value === "url" ? (
                        <>
                          <Tabs.Content value={"url"}>
                            <Label htmlFor={"clone-url"}>
                              {"Repository URL"}
                            </Label>
                            <Input
                              id={"clone-url"}
                              value={cloneUrl.value}
                              placeholder={"https://github.com/owner/repo.git"}
                              spellCheck={false}
                              autoComplete={"off"}
                              disabled={busy.value}
                              onInput={(event) =>
                                changeCloneUrl(event.currentTarget.value)
                              }
                            />
                            <small className={cn("clone-auth-note")}>
                              {
                                "Paste an HTTPS or SSH clone URL from your Git host."
                              }
                            </small>
                          </Tabs.Content>
                        </>
                      ) : (
                        <>
                          <Tabs.Content value={"account"}>
                            {!sorted.value.length ? (
                              <>
                                <div className={cn("empty-small")}>
                                  {
                                    "Connect a GitHub account to choose a repository."
                                  }
                                </div>
                                <Button
                                  variant={"primary"}
                                  onClick={onconnect.value}
                                >
                                  {"Connect GitHub"}
                                </Button>
                              </>
                            ) : (
                              <>
                                <div className={cn("account-repositories")}>
                                  {(() => {
                                    const entries = sorted.value;
                                    return entries.map((account, index) => (
                                      <Fragment key={account.id}>
                                        <section>
                                          <Button
                                            aria-expanded={
                                              expanded.value === account.id
                                            }
                                            disabled={busy.value}
                                            onClick={() => {
                                              if (
                                                expanded.value === account.id
                                              ) {
                                                changeCatalogQuery("");
                                                expanded.value = null;
                                                ++sequence;
                                                listLoading.value = false;
                                              } else void expand(account.id);
                                            }}
                                            className={cn("account-group")}
                                          >
                                            <PlatformIcon
                                              remote={"https://github.com"}
                                              size={20}
                                            />
                                            <strong>
                                              {"@"}
                                              {account.login}
                                            </strong>
                                            <ChevronDown size={16} />
                                          </Button>
                                          {expanded.value === account.id ? (
                                            <>
                                              <div
                                                className={cn(
                                                  "account-repo-list",
                                                )}
                                              >
                                                <Input
                                                  aria-label={
                                                    "Find repositories for " +
                                                    account.login
                                                  }
                                                  placeholder={
                                                    "Search this account"
                                                  }
                                                  value={query.value}
                                                  disabled={busy.value}
                                                  onValueChange={changeCatalogQuery}
                                                />
                                                {(searching.value ? catalogError.value : listError.value) ? (
                                                  <>
                                                    <p
                                                      role={"alert"}
                                                      className={cn(
                                                        "connection-error",
                                                      )}
                                                    >
                                                      {searching.value ? catalogError.value : listError.value}
                                                    </p>
                                                    <Button
                                                      onClick={() => searching.value ? searchAccount(account.id) : expand(account.id, page.value)}
                                                      className={cn(
                                                        "text-action",
                                                      )}
                                                    >
                                                      {"Try again"}
                                                    </Button>
                                                  </>
                                                ) : null}
                                                {(searching.value ? catalogLoading.value : listLoading.value) ? (
                                                  <>
                                                    <p
                                                      role={"status"}
                                                      className={cn(
                                                        "empty-small",
                                                      )}
                                                    >
                                                      {searching.value ? "Searching all repositories for this account…" : "Loading repositories…"}
                                                    </p>
                                                  </>
                                                ) : (
                                                  <>
                                                    {(() => {
                                                      const entries =
                                                        displayed.value;
                                                      return entries.length ===
                                                        0 ? (
                                                        <>
                                                          {!(searching.value ? catalogError.value : listError.value) ? (
                                                            <>
                                                              <p
                                                                className={cn(
                                                                  "empty-small",
                                                                )}
                                                              >
                                                                {
                                                                  searching.value ? "No matching repositories in this account." : "No repositories on this page."
                                                                }
                                                              </p>
                                                            </>
                                                          ) : null}
                                                        </>
                                                      ) : (
                                                        entries.map(
                                                          (repo) => (
                                                            <Fragment
                                                              key={repo.id}
                                                            >
                                                              <Button
                                                                disabled={
                                                                  busy.value
                                                                }
                                                                onClick={() => {
                                                                  selected.value =
                                                                    {
                                                                      accountId:
                                                                        account.id,
                                                                      repo,
                                                                    };
                                                                  name.value =
                                                                    repo.name;
                                                                }}
                                                                className={cn([
                                                                  "online-repo",
                                                                  {
                                                                    "is-selected":
                                                                      selected
                                                                        .value
                                                                        ?.accountId ===
                                                                        account.id &&
                                                                      selected
                                                                        .value
                                                                        .repo
                                                                        .id ===
                                                                        repo.id,
                                                                  },
                                                                ])}
                                                              >
                                                                <span>
                                                                  {
                                                                    repo.full_name
                                                                  }
                                                                </span>
                                                                {alreadyAdded(repo) && (
                                                                  <span className="online-repo-icon online-repo-added" title="Already added">
                                                                    <CircleCheck size={14} aria-hidden="true" />
                                                                    <span>Already added</span>
                                                                  </span>
                                                                )}
                                                                <span className="online-repo-icon" title={repo.private ? "Private repository" : "Public repository"}>
                                                                  {repo.private ? <Lock size={13} aria-hidden="true" /> : <Globe size={13} aria-hidden="true" />}
                                                                  <span className="sr-only">{repo.private ? "Private repository" : "Public repository"}</span>
                                                                </span>
                                                                {selected.value
                                                                  ?.accountId ===
                                                                  account.id &&
                                                                selected.value
                                                                  .repo.id ===
                                                                  repo.id ? (
                                                                  <>
                                                                    <Check
                                                                      size={17}
                                                                    />
                                                                  </>
                                                                ) : null}
                                                              </Button>
                                                            </Fragment>
                                                          ),
                                                        )
                                                      );
                                                    })()}
                                                    <div
                                                      className={cn(
                                                        "history-paging",
                                                      )}
                                                    >
                                                      <Button
                                                        variant={"secondary"}
                                                        disabled={
                                                          busy.value ||
                                                          (searching.value ? searchPage.value : page.value) === 1
                                                        }
                                                        onClick={() => searching.value ? searchPage.value-- : expand(account.id, page.value - 1)}
                                                      >
                                                        {"Previous"}
                                                      </Button>
                                                      <span>{searching.value ? searchPage.value : page.value}</span>
                                                      <Button
                                                        variant={"secondary"}
                                                        disabled={
                                                          busy.value ||
                                                          (searching.value ? searchPage.value * browseSize >= filtered.value.length : !hasMore.value)
                                                        }
                                                        onClick={() => searching.value ? searchPage.value++ : expand(account.id, page.value + 1)}
                                                      >
                                                        {"Next"}
                                                      </Button>
                                                    </div>
                                                  </>
                                                )}
                                              </div>
                                            </>
                                          ) : null}
                                        </section>
                                      </Fragment>
                                    ));
                                  })()}
                                </div>
                              </>
                            )}
                          </Tabs.Content>
                        </>
                      )}
                    </Tabs.Root>
                    {cloneSource.value === "url" || selected.value ? (
                      <>
                        {cloneSource.value === "account" && selected.value ? (
                          <>
                            <p className={cn("selected-clone")}>
                              <Check size={15} />
                              {selected.value.repo.full_name}
                            </p>
                          </>
                        ) : null}
                        <Label htmlFor={"clone-name"}>
                          {"Local folder name"}
                        </Label>
                        <Input
                          id={"clone-name"}
                          value={name.value}
                          maxLength={100}
                          disabled={busy.value}
                          onValueChange={(next) => {
                            name.value = next;
                          }}
                        />
                        <Label htmlFor={"clone-parent"}>{"Save inside"}</Label>
                        <div className={cn("folder-picker")}>
                          <Input
                            id={"clone-parent"}
                            value={parent.value}
                            placeholder={"Choose a parent folder"}
                            disabled={busy.value}
                            onValueChange={(next) => {
                              parent.value = next;
                            }}
                          />
                          <Button
                            variant={"secondary"}
                            disabled={busy.value}
                            onClick={() => browse("parent")}
                          >
                            <FolderOpen size={16} />
                            {" Browse…"}
                          </Button>
                        </div>
                        <small className={cn("clone-auth-note")}>
                          {cloneSource.value === "account"
                            ? selected.value?.repo.private
                              ? "Clone uses the selected GitHub account."
                              : "Public repositories clone without authentication."
                            : "Private repositories use your system Git credentials or SSH key."}
                        </small>
                      </>
                    ) : null}
                    {cloning.value ? (
                      <>
                        <p role={"status"} className={cn("clone-auth-note")}>
                          {"Cloning repository… This may take a few minutes."}
                        </p>
                      </>
                    ) : null}
                    <div className={cn("modal-actions")}>
                      <Button
                        variant={"secondary"}
                        disabled={busy.value}
                        onClick={onclose.value}
                      >
                        {"Cancel"}
                      </Button>
                      <Button
                        variant={"primary"}
                        disabled={
                          busy.value ||
                          (cloneSource.value === "url"
                            ? !cloneUrl.value.trim()
                            : !selected.value) ||
                          !parent.value.trim() ||
                          !name.value.trim()
                        }
                        onClick={submit}
                      >
                        {busy.value ? (
                          <>
                            <LoaderCircle size={15} className={cn("spin")} />
                          </>
                        ) : null}
                        {" Clone repository"}
                      </Button>
                    </div>
                  </>
                ) : (
                  <>
                    <Label htmlFor={"create-name"}>{"Repository name"}</Label>
                    <Input
                      id={"create-name"}
                      value={name.value}
                      placeholder={"my-project"}
                      maxLength={100}
                      disabled={busy.value}
                      onValueChange={(next) => {
                        name.value = next;
                      }}
                    />
                    <Label htmlFor={"create-account"}>{"Account"}</Label>
                    <NativeSelect
                      id={"create-account"}
                      value={accountId.value}
                      disabled={busy.value}
                      onValueChange={(next) => {
                        accountId.value = next;
                      }}
                      onChange={() => loadOwners()}
                    >
                      <SelectOption value={0}>{"Local only"}</SelectOption>
                      {(() => {
                        const entries = sorted.value;
                        return entries.map((account, index) => (
                          <Fragment key={account.id}>
                            <SelectOption value={account.id}>
                              {"GitHub · @"}
                              {account.login}
                            </SelectOption>
                          </Fragment>
                        ));
                      })()}
                    </NativeSelect>
                    {accountId.value ? (
                      <>
                        <Label htmlFor={"create-owner"}>{"Owner"}</Label>
                        <NativeSelect
                          id={"create-owner"}
                          value={owner.value}
                          disabled={busy.value || ownersLoading.value}
                          onValueChange={(next) => {
                            owner.value = next;
                          }}
                        >
                          {ownersLoading.value ? (
                            <>
                              <SelectOption value={""}>
                                {"Loading owners…"}
                              </SelectOption>
                            </>
                          ) : null}
                          {(() => {
                            const entries = owners.value;
                            return entries.map((item, index) => (
                              <Fragment key={item.login}>
                                <SelectOption value={item.login}>
                                  {item.login}
                                  {item.type === "Organization"
                                    ? " · Organization"
                                    : " · Personal"}
                                </SelectOption>
                              </Fragment>
                            ));
                          })()}
                        </NativeSelect>
                        {moreOwners.value ? (
                          <>
                            <Button
                              disabled={busy.value || ownersLoading.value}
                              onClick={() => loadOwners(ownerPage.value + 1)}
                              className={cn("text-action")}
                            >
                              {"Load more organizations"}
                            </Button>
                          </>
                        ) : null}
                        {ownerWarning.value ? (
                          <>
                            <p
                              role={"alert"}
                              className={cn("connection-error")}
                            >
                              {ownerWarning.value}
                            </p>
                          </>
                        ) : null}
                        <Label htmlFor={"create-visibility"}>
                          {"Visibility"}
                        </Label>
                        <NativeSelect
                          id={"create-visibility"}
                          value={isPrivate.value}
                          disabled={busy.value}
                          onValueChange={(next) => {
                            isPrivate.value = next;
                          }}
                        >
                          <SelectOption value={true}>{"Private"}</SelectOption>
                          <SelectOption value={false}>{"Public"}</SelectOption>
                        </NativeSelect>
                      </>
                    ) : null}
                    <Label htmlFor={"create-parent"}>{"Save inside"}</Label>
                    <div className={cn("folder-picker")}>
                      <Input
                        id={"create-parent"}
                        value={parent.value}
                        placeholder={"Choose a parent folder"}
                        disabled={busy.value}
                        onValueChange={(next) => {
                          parent.value = next;
                        }}
                      />
                      <Button
                        variant={"secondary"}
                        disabled={busy.value}
                        onClick={() => browse("parent")}
                      >
                        <FolderOpen size={16} />
                        {" Browse…"}
                      </Button>
                    </div>
                    <p className={cn("create-summary")}>
                      {accountId.value
                        ? (isPrivate.value ? "Private" : "Public") +
                          " repository · " +
                          (owner.value || "Choose owner")
                        : "Local repository · main branch"}
                    </p>
                    <div className={cn("modal-actions")}>
                      <Button
                        variant={"secondary"}
                        disabled={busy.value}
                        onClick={onclose.value}
                      >
                        {"Cancel"}
                      </Button>
                      <Button
                        variant={"primary"}
                        disabled={
                          busy.value ||
                          !name.value.trim() ||
                          !parent.value.trim() ||
                          (accountId.value !== 0 &&
                            (!owner.value || ownersLoading.value))
                        }
                        onClick={submit}
                      >
                        {busy.value ? (
                          <>
                            <LoaderCircle size={15} className={cn("spin")} />
                          </>
                        ) : null}
                        {accountId.value
                          ? "Create on GitHub"
                          : "Create repository"}
                      </Button>
                    </div>
                  </>
                )}
              </>
            )}
          </>
        )}
      </Modal>
    </>
  );
}
export default observer(function AddRepository(props: Props) {
  return useController(createView, props);
});
