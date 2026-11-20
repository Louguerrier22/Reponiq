import {
Check,
CircleAlert,
FileCode,
GitPullRequest,
Plus,
RefreshCw,
Search,
} from "lucide-react";
import { observer } from "mobx-react-lite";
import { Fragment } from "react";
import {
api,
type Account,
type GitState,
type Pull,
type PullCommit,
type PullComparison,
type PullContext,
type PullDetails,
type PullFile,
type PullMergeScope,
type RepoWorkflow,
type ReviewedPull,
type Target,
} from "../../lib/api";
import {
draftKey,
readDraft,
saveDraft,
type DraftContext,
} from "../../lib/draft-store";
import { confirm } from "../confirmation";
import {
SelectOption,
derived,
snapshot,
state,
untrack,
useController,
type Lifecycle
} from "../controller";
import { Button,Card,Checkbox,Collapsible,Input,Label,NativeSelect,Textarea } from "../ui";
import { cn } from "../utils";
import DiffView from "./DiffView";
import Modal from "./Modal";
type Props = {
  target: Target;
  git: GitState | null;
  accounts: Account[];
  preferences?: RepoWorkflow;
  persistentDrafts: boolean;
  onconnect: () => void;
  onchanged: () => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const target = derived(() => lifecycle.props().target);
  const git = derived(() => lifecycle.props().git);
  const accounts = derived(() => lifecycle.props().accounts);
  const preferences = derived(() => lifecycle.props().preferences);
  const persistentDrafts = derived(() => lifecycle.props().persistentDrafts);
  const onconnect = derived(() => lifecycle.props().onconnect);
  const onchanged = derived(() => lifecycle.props().onchanged);
  const repoId = derived(() => target.value.id);
  let accountId = state(0),
    page = state(1),
    hasMore = state(false),
    rows = state<Pull[]>([]),
    context = state<PullContext | null>(null),
    selected = state<Pull | null>(null),
    detail = state<PullDetails | null>(null);
  let loading = state(false),
    detailLoading = state(false),
    busy = state(false),
    error = state(""),
    notice = state(""),
    query = state("");
  let files = state<PullFile[]>([]),
    filePage = state(1),
    moreFiles = state(false),
    fileQuery = state(""),
    selectedFile = state<PullFile | null>(null),
    filesLoading = state(false);
  let commits = state<PullCommit[]>([]),
    commitsLimited = state(false),
    commit = state("");
  let reviewEvent = state("COMMENT"),
    reviewBody = state(""),
    reviewQuery = state(""),
    reviewPage = state(0),
    checkQuery = state(""),
    checkPage = state(0),
    method = state("merge");
  let showCreate = state(false),
    title = state(""),
    body = state(""),
    base = state(""),
    draft = state(false);
  let accountPicked = state(false),
    draftLoading = state(false),
    draftVersion = 0;
  const account = derived(() =>
    accounts.value.find((account) => account.id === accountId.value),
  );
  const composeScope = derived((): DraftContext | null => {
    if (
      !showCreate.value ||
      !git.value?.branch ||
      git.value.branch === "(detached)" ||
      !account.value ||
      !base.value.trim()
    )
      return null;
    return {
      target: target.value,
      scope: {
        branch: git.value.branch,
        account: account.value.login,
        base: base.value.trim(),
      },
    };
  });
  const composeKey = derived(
    () => composeScope.value && draftKey(composeScope.value),
  );
  let comparison = state<{
    repo: number;
    account: number;
    base: string;
    branch: string;
    head: string;
    result: PullComparison;
  } | null>(null);
  let comparing = state(false);
  let pendingMerge = state<{
    review: ReviewedPull;
    method: string;
    title: string;
  } | null>(null);
  let detailScope = state<ReviewedPull | null>(null);
  let listSequence = 0,
    detailSequence = 0,
    fileSequence = 0,
    alive = true;
  const filtered = derived(() =>
    rows.value.filter((p) =>
      [p.title, p.user.login, String(p.number), p.head.ref, p.base.ref].some(
        (s) => s.toLocaleLowerCase().includes(query.value.toLocaleLowerCase()),
      ),
    ),
  );
  const visibleFiles = derived(() =>
    files.value.filter((f) =>
      f.filename
        .toLocaleLowerCase()
        .includes(fileQuery.value.toLocaleLowerCase()),
    ),
  );
  const reviews = derived(() =>
    (detail.value?.reviews ?? [])
      .filter((r) =>
        [r.user.login, r.state, r.body ?? ""].some((s) =>
          s.toLocaleLowerCase().includes(reviewQuery.value.toLocaleLowerCase()),
        ),
      )
      .slice()
      .reverse(),
  );
  const checks = derived(() =>
    (detail.value?.checks.runs ?? []).filter((c) =>
      [c.name, c.status, c.conclusion ?? ""].some((s) =>
        s.toLocaleLowerCase().includes(checkQuery.value.toLocaleLowerCase()),
      ),
    ),
  );
  const blockedReview = derived(() => {
    const latest = new Map<string, string>();
    for (const r of detail.value?.reviews ?? [])
      if (["APPROVED", "CHANGES_REQUESTED", "DISMISSED"].includes(r.state))
        latest.set(r.user.login, r.state);
    return [...latest.values()].includes("CHANGES_REQUESTED");
  });
  function matchesMergeScope(scope: PullMergeScope | null) {
    return (
      !!scope?.target.registration &&
      scope.target.id === target.value.id &&
      scope.target.path === target.value.path &&
      scope.target.registration === target.value.registration &&
      scope.accountId === accountId.value &&
      scope.accountLogin === account.value?.login
    );
  }
  const canMerge = derived(
    () =>
      !busy.value &&
      !detailLoading.value &&
      matchesMergeScope(detailScope.value) &&
      !!detail.value &&
      detail.value.pull.state === "open" &&
      !detail.value.pull.draft &&
      detail.value.pull.mergeable === true &&
      ["clean", "has_hooks"].includes(
        detail.value.pull.mergeable_state ?? "",
      ) &&
      detail.value.checks.ready &&
      detail.value.checks.complete &&
      detail.value.reviews_complete &&
      !blockedReview.value &&
      detail.value.merge_methods.includes(method.value),
  );
  const canCompare = derived(
    () =>
      !busy.value &&
      !comparing.value &&
      !!git.value?.head &&
      !git.value.dirty &&
      !git.value.operation &&
      !git.value.conflicts.length &&
      !!git.value.upstream &&
      !git.value.upstream_gone &&
      git.value.ahead === 0 &&
      !!base.value.trim() &&
      git.value.branch !== base.value,
  );
  const currentComparison = derived(() =>
    comparison.value?.repo === repoId.value &&
    comparison.value?.account === accountId.value &&
    comparison.value?.base === base.value &&
    comparison.value?.head === git.value?.head &&
    comparison.value?.branch === git.value?.branch
      ? comparison.value.result
      : null,
  );
  const canCreate = derived(
    () =>
      canCompare.value &&
      !draftLoading.value &&
      !!currentComparison.value?.ahead_by,
  );
  lifecycle.effect(() => {
    const list = accounts.value;
    const remembered = preferences.value?.pr_login;
    if (accountPicked.value) {
      if (accountId.value && !list.some((a) => a.id === accountId.value))
        accountId.value = 0;
      return;
    }
    accountId.value = remembered
      ? (list.find(
          (a) => a.login.toLocaleLowerCase() === remembered.toLocaleLowerCase(),
        )?.id ?? 0)
      : list.length === 1
        ? list[0].id
        : 0;
  });
  lifecycle.effect(() => {
    repoId.value;
    accountId.value;
    untrack(() => {
      if (accountId.value) void loadList(1);
      else {
        loading.value = false;
        rows.value = [];
        context.value = null;
        selected.value = null;
        detail.value = null;
        ++listSequence;
        ++detailSequence;
        ++fileSequence;
      }
    });
  });
  lifecycle.effect(() => {
    const key = composeKey.value;
    if (key) void untrack(() => restoreDraft(key));
    else {
      ++draftVersion;
      title.value = "";
      body.value = "";
      draftLoading.value = false;
    }
  });
  lifecycle.effect(() => {
    reviewQuery.value;
    reviewPage.value = 0;
  });
  lifecycle.effect(() => {
    checkQuery.value;
    checkPage.value = 0;
  });
  lifecycle.destroy(() => {
    alive = false;
    ++listSequence;
    ++detailSequence;
    ++fileSequence;
  });
  async function loadList(next: number) {
    if (!accountId.value) return;
    const sequence = ++listSequence;
    const id = repoId.value,
      account = accountId.value;
    loading.value = true;
    error.value = "";
    selected.value = null;
    detail.value = null;
    files.value = [];
    commits.value = [];
    ++detailSequence;
    ++fileSequence;
    const results = await Promise.allSettled([
      api.pulls(id, account, next),
      api.pullContext(id, account),
    ]);
    if (!alive || sequence !== listSequence) return;
    if (results[0].status === "fulfilled") {
      rows.value = results[0].value.items;
      hasMore.value = results[0].value.has_more;
      page.value = next;
      if (rows.value[0]) void choose(rows.value[0]);
    } else {
      rows.value = [];
      error.value = String(results[0].reason);
    }
    if (results[1].status === "fulfilled") {
      context.value = results[1].value;
    } else {
      context.value = null;
      error.value = String(results[1].reason);
    }
    loading.value = false;
  }
  async function choose(pull: Pull) {
    const sequence = ++detailSequence;
    const same = selected.value?.number === pull.number;
    selected.value = pull;
    detail.value = null;
    detailScope.value = null;
    files.value = [];
    selectedFile.value = null;
    commits.value = [];
    commitsLimited.value = false;
    commit.value = "";
    detailLoading.value = true;
    error.value = "";
    ++fileSequence;
    if (!same) {
      reviewBody.value = "";
      reviewEvent.value = "COMMENT";
    }
    reviewPage.value = 0;
    checkPage.value = 0;
    const scope = {
      target: { ...target.value },
      accountId: accountId.value,
      accountLogin: account.value?.login ?? "",
    };
    try {
      const result = await api.pullDetails(scope, pull.number);
      if (!alive || sequence !== detailSequence || !matchesMergeScope(scope))
        return;
      const reviewed: ReviewedPull = {
        ...scope,
        repository: result.repository,
        repositoryId: result.repository_id,
        number: result.pull.number,
        head: result.pull.head.sha,
        base: result.pull.base.ref,
      };
      detailScope.value = reviewed;
      detail.value = result;
      selected.value = result.pull;
      method.value = result.merge_methods[0] ?? "merge";
      void loadFiles(1);
      const page = await api.pullCommits(reviewed);
      if (alive && sequence === detailSequence && matchesMergeScope(reviewed)) {
        commits.value = page.items;
        commitsLimited.value = page.has_more;
      }
    } catch (e) {
      if (alive && sequence === detailSequence) error.value = String(e);
    } finally {
      if (alive && sequence === detailSequence) detailLoading.value = false;
    }
  }
  async function loadFiles(next: number) {
    const reviewed = detailScope.value;
    if (!reviewed || !matchesMergeScope(reviewed)) return;
    const sequence = ++fileSequence;
    const sha = commit.value || null;
    filesLoading.value = true;
    selectedFile.value = null;
    fileQuery.value = "";
    try {
      const result = await api.pullFiles(reviewed, next, sha);
      if (alive && sequence === fileSequence && matchesMergeScope(reviewed)) {
        files.value = result.items;
        moreFiles.value = result.has_more;
        filePage.value = next;
        selectedFile.value = files.value[0] ?? null;
      }
    } catch (e) {
      if (alive && sequence === fileSequence) {
        files.value = [];
        error.value = String(e);
      }
    } finally {
      if (alive && sequence === fileSequence) filesLoading.value = false;
    }
  }
  function openCreate() {
    error.value = "";
    title.value = "";
    body.value = "";
    draft.value = false;
    comparison.value = null;
    const preferred = preferences.value?.preferred_base;
    base.value = preferred
      ? context.value?.branches.includes(preferred)
        ? preferred
        : ""
      : (context.value?.default_branch ?? "");
    if (preferred && !base.value)
      error.value =
        "Your preferred base is unavailable in the branch list. Choose or enter a base.";
    showCreate.value = true;
  }
  async function restoreDraft(key: string) {
    const scope = composeScope.value;
    if (!scope) return;
    const revision = ++draftVersion;
    title.value = "";
    body.value = "";
    draftLoading.value = true;
    try {
      const value = await readDraft(scope, persistentDrafts.value);
      if (alive && revision === draftVersion && composeKey.value === key) {
        title.value = value?.title ?? "";
        body.value = value?.body ?? "";
      }
    } catch (reason) {
      if (alive && revision === draftVersion) error.value = String(reason);
    } finally {
      if (alive && revision === draftVersion) draftLoading.value = false;
    }
  }
  function persistDraft() {
    if (composeScope.value)
      void saveDraft(
        composeScope.value,
        { title: title.value, body: body.value },
        persistentDrafts.value,
      ).catch((reason) => {
        error.value = String(reason);
      });
  }
  async function discardDraft() {
    const scope = composeScope.value;
    if (
      !scope ||
      !(await confirm({
        title: "Discard this pull request draft?",
        action: "Discard draft",
        destructive: true,
      })) ||
      scope !== composeScope.value
    )
      return;
    title.value = "";
    body.value = "";
    persistDraft();
  }
  async function openFailedCheck(number: number, webUrl: string | null) {
    try {
      await api.openCheck(repoId.value, number, webUrl);
    } catch (reason) {
      error.value = String(reason);
    }
  }
  async function compare() {
    if (!canCompare.value || !git.value?.head) return;
    const snapshot = {
      repo: repoId.value,
      account: accountId.value,
      base: base.value,
      branch: git.value.branch,
      head: git.value.head,
    };
    comparing.value = true;
    error.value = "";
    comparison.value = null;
    try {
      const result = await api.comparePull(
        snapshot.repo,
        snapshot.account,
        snapshot.base,
        snapshot.branch,
        snapshot.head,
      );
      if (alive && showCreate.value) comparison.value = { ...snapshot, result };
    } catch (reason) {
      if (alive) error.value = String(reason);
    } finally {
      comparing.value = false;
    }
  }
  async function create() {
    if (
      !git.value?.head ||
      !canCreate.value ||
      !title.value.trim() ||
      !currentComparison.value
    )
      return;
    const input = {
      title: title.value,
      body: body.value,
      base: base.value,
      head: git.value.branch,
      expected_head: git.value.head,
      expected_base: currentComparison.value.base_commit.sha,
      draft: draft.value,
    };
    const id = repoId.value,
      account = accountId.value;
    const scope = composeScope.value;
    busy.value = true;
    error.value = "";
    try {
      const { pull, matches_comparison } = await api.createPull(
        id,
        account,
        input,
      );
      notice.value = matches_comparison
        ? "Created pull request #" + pull.number
        : "";
      showCreate.value = false;
      title.value = "";
      body.value = "";
      let draftError = "";
      if (scope && matches_comparison)
        await saveDraft(scope, null, persistentDrafts.value).catch((reason) => {
          draftError = String(reason);
        });
      await loadList(1);
      await choose(pull);
      if (!matches_comparison)
        error.value = `GitHub created pull request #${pull.number} after a branch changed. Review the new pull request before continuing.`;
      else if (draftError) error.value = draftError;
    } catch (reason) {
      comparison.value = null;
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  async function review() {
    if (!detail.value || busy.value || detailLoading.value) return;
    const number = detail.value.pull.number,
      head = detail.value.pull.head.sha,
      id = repoId.value,
      account = accountId.value,
      event = reviewEvent.value,
      text = reviewBody.value;
    busy.value = true;
    error.value = "";
    try {
      await api.reviewPull(id, account, number, head, event, text);
      notice.value = "Review submitted for #" + number;
      reviewBody.value = "";
      if (selected.value?.number === number) await choose(selected.value);
    } catch (e) {
      error.value = String(e);
    } finally {
      busy.value = false;
    }
  }
  function confirmMerge() {
    if (!detail.value || !detailScope.value || !canMerge.value) return;
    pendingMerge.value = {
      review: snapshot(detailScope.value),
      method: method.value,
      title: detail.value.pull.title,
    };
  }
  async function merge() {
    if (!pendingMerge.value || busy.value) return;
    if (!matchesMergeScope(pendingMerge.value.review)) {
      pendingMerge.value = null;
      error.value =
        "The repository or GitHub account changed. Reopen the pull request before merging.";
      return;
    }
    const { review, method } = pendingMerge.value;
    busy.value = true;
    error.value = "";
    try {
      notice.value =
        (await api.mergePull(review, method)) +
        " Fetch to update your local refs.";
      pendingMerge.value = null;
      onchanged.value();
      await loadList(1);
    } catch (e) {
      const message = String(e);
      pendingMerge.value = null;
      if (selected.value) await choose(selected.value);
      error.value = message;
    } finally {
      busy.value = false;
    }
  }
  return () => {
    const compared = currentComparison.value;
    return (
      <>
        <section aria-label={"Pull requests"} className={cn("pull-requests")}>
          <div className={cn("pr-toolbar")}>
            <Label>
              {"GitHub account"}
              <NativeSelect
                aria-label={"GitHub account for pull requests"}
                value={accountId.value}
                disabled={busy.value || loading.value}
                onValueChange={(next) => {
                  accountId.value = next;
                }}
                onChange={() => {
                  accountPicked.value = true;
                }}
              >
                <SelectOption value={0}>{"Choose an account"}</SelectOption>
                {(() => {
                  const entries = accounts.value;
                  return entries.map((account, index) => (
                    <Fragment key={index}>
                      <SelectOption value={account.id}>
                        {"@"}
                        {account.login}
                      </SelectOption>
                    </Fragment>
                  ));
                })()}
              </NativeSelect>
            </Label>
            <div className={cn("toolbar")}>
              <Button
                variant={"secondary"}
                disabled={busy.value || loading.value || !accountId.value}
                onClick={() => loadList(page.value)}
              >
                <RefreshCw size={15} />
                {" Refresh"}
              </Button>
              <Button
                variant={"primary"}
                disabled={!accountId.value || !context.value || busy.value}
                onClick={openCreate}
              >
                <Plus size={15} />
                {" New pull request"}
              </Button>
            </div>
          </div>
          {notice.value ? (
            <>
              <p role={"status"} className={cn("inline-success")}>
                {notice.value}
              </p>
            </>
          ) : null}
          {error.value ? (
            <>
              <p role={"alert"} className={cn("connection-error")}>
                {error.value}
              </p>
            </>
          ) : null}
          {accounts.value.length && !accountId.value ? (
            <>
              <p>
                {"Select a connected GitHub account to view pull requests."}
              </p>
            </>
          ) : null}
          {!accounts.value.length ? (
            <>
              <div className={cn("empty-block")}>
                <GitPullRequest size={28} />
                <p>{"Connect GitHub to review and manage pull requests."}</p>
                <Button variant={"primary"} onClick={onconnect.value}>
                  {"Connect GitHub"}
                </Button>
              </div>
            </>
          ) : (
            <>
              <div className={cn("pr-workbench")}>
                <Card as={"section"} className={cn("panel pr-list")}>
                  <div className={cn("panel-title")}>
                    {"Open pull requests "}
                    <span>{context.value?.full_name ?? ""}</span>
                  </div>
                  <div className={cn("search-input history-search")}>
                    <Search size={15} />
                    <Input
                      aria-label={"Find pull requests"}
                      placeholder={"Find in this page"}
                      value={query.value}
                      onValueChange={(next) => {
                        query.value = next;
                      }}
                    />
                  </div>
                  <div className={cn("commit-list")}>
                    {(() => {
                      const entries = filtered.value;
                      return entries.length === 0 ? (
                        <>
                          <p className={cn("empty-small")}>
                            {loading.value
                              ? "Loading pull requests…"
                              : query.value
                                ? "No matching pull requests"
                                : "No open pull requests"}
                          </p>
                        </>
                      ) : (
                        entries.map((pull, index) => (
                          <Fragment key={pull.number}>
                            <Button
                              disabled={busy.value}
                              onClick={() => {
                                if (selected.value?.number !== pull.number)
                                  void choose(pull);
                              }}
                              className={cn([
                                "commit-choice",
                                {
                                  "is-selected":
                                    selected.value?.number === pull.number,
                                },
                              ])}
                            >
                              <GitPullRequest size={18} />
                              <span>
                                <strong>{pull.title}</strong>
                                <small>
                                  {"#"}
                                  {pull.number}
                                  {" · @"}
                                  {pull.user.login}
                                  {pull.draft ? " · Draft" : ""}
                                </small>
                                <code>
                                  {pull.head.ref}
                                  {" → "}
                                  {pull.base.ref}
                                </code>
                              </span>
                            </Button>
                          </Fragment>
                        ))
                      );
                    })()}
                  </div>
                  <div className={cn("history-paging")}>
                    <Button
                      variant={"secondary"}
                      disabled={busy.value || loading.value || page.value === 1}
                      onClick={() => loadList(page.value - 1)}
                    >
                      {"Previous"}
                    </Button>
                    <span>{page.value}</span>
                    <Button
                      variant={"secondary"}
                      disabled={busy.value || loading.value || !hasMore.value}
                      onClick={() => loadList(page.value + 1)}
                    >
                      {"Next"}
                    </Button>
                  </div>
                </Card>
                <div className={cn("pr-inspector")}>
                  {detail.value && selected.value ? (
                    <>
                      <Card as={"section"} className={cn("panel pr-summary")}>
                        <div className={cn("pr-title")}>
                          <div>
                            <h2>
                              {"#"}
                              {detail.value.pull.number}{" "}
                              {detail.value.pull.title}
                            </h2>
                            <p>
                              {"@"}
                              {detail.value.pull.user.login}
                              {" · "}
                              {detail.value.pull.head.label}
                              {" → "}
                              {detail.value.pull.base.ref}
                            </p>
                          </div>
                          <span
                            className={cn(
                              "pill",
                              detail.value.pull.mergeable === true &&
                                !detail.value.pull.draft &&
                                "green",
                              (detail.value.pull.draft ||
                                detail.value.pull.mergeable !== true) &&
                                "yellow",
                            )}
                          >
                            {detail.value.pull.draft
                              ? "Draft"
                              : detail.value.pull.mergeable === false
                                ? "Conflicts"
                                : detail.value.pull.mergeable === null
                                  ? "Checking"
                                  : "Open"}
                          </span>
                        </div>
                        {detail.value.pull.body ? (
                          <>
                            <pre className={cn("pr-description")}>
                              {detail.value.pull.body.slice(0, 20000)}
                            </pre>
                          </>
                        ) : null}
                        <code className={cn("pr-head")}>
                          {"Reviewed head: "}
                          {detail.value.pull.head.sha.slice(0, 12)}
                        </code>
                        <Collapsible
                          className={cn("pr-status")}
                          heading={() => (
                            <>
                              {detail.value?.checks.ready ? (
                                <>
                                  <Check size={15} />
                                  {" Checks passed or not configured"}
                                </>
                              ) : (
                                <>
                                  <CircleAlert size={15} />
                                  {" Checks pending, failing or unavailable"}
                                </>
                              )}
                            </>
                          )}
                        >
                          {detail.value.checks.runs.length > 10 ? (
                            <>
                              <Input
                                aria-label={"Filter checks"}
                                placeholder={"Filter checks"}
                                value={checkQuery.value}
                                onValueChange={(next) => {
                                  checkQuery.value = next;
                                }}
                              />
                            </>
                          ) : null}
                          <div className={cn("check-list")}>
                            {(() => {
                              const entries = checks.value.slice(
                                checkPage.value * 10,
                                checkPage.value * 10 + 10,
                              );
                              return entries.map((check, index) => (
                                <Fragment key={index}>
                                  <div>
                                    <span>{check.name}</span>
                                    <b
                                      className={cn(
                                        check.conclusion === "success" &&
                                          "change-added",
                                        [
                                          "failure",
                                          "timed_out",
                                          "cancelled",
                                        ].includes(check.conclusion ?? "") &&
                                          "change-deleted",
                                      )}
                                    >
                                      {check.conclusion ?? check.status}
                                    </b>
                                    {check.conclusion &&
                                    !["success", "neutral", "skipped"].includes(
                                      check.conclusion,
                                    ) ? (
                                      <>
                                        <Button
                                          onClick={() =>
                                            detail.value &&
                                            openFailedCheck(
                                              detail.value.pull.number,
                                              check.web_url ?? null,
                                            )
                                          }
                                          className={cn("text-action")}
                                        >
                                          {"Open check"}
                                        </Button>
                                      </>
                                    ) : null}
                                  </div>
                                </Fragment>
                              ));
                            })()}
                            <p>
                              {"Commit status: "}
                              {detail.value.checks.legacy_state}
                            </p>
                            {!detail.value.checks.ready ? (
                              <>
                                <Button
                                  onClick={() =>
                                    detail.value &&
                                    openFailedCheck(
                                      detail.value.pull.number,
                                      null,
                                    )
                                  }
                                  className={cn("text-action")}
                                >
                                  {"Open checks on GitHub"}
                                </Button>
                              </>
                            ) : null}
                            {!detail.value.checks.complete ? (
                              <>
                                <p>
                                  {
                                    "Check results are incomplete. Merge is blocked."
                                  }
                                </p>
                              </>
                            ) : null}
                          </div>
                          {checks.value.length > 10 ? (
                            <>
                              <div className={cn("history-paging")}>
                                <Button
                                  variant={"secondary"}
                                  disabled={checkPage.value === 0}
                                  onClick={() => checkPage.value--}
                                >
                                  {"Previous"}
                                </Button>
                                <span>{checkPage.value + 1}</span>
                                <Button
                                  variant={"secondary"}
                                  disabled={
                                    (checkPage.value + 1) * 10 >=
                                    checks.value.length
                                  }
                                  onClick={() => checkPage.value++}
                                >
                                  {"Next"}
                                </Button>
                              </div>
                            </>
                          ) : null}
                        </Collapsible>
                      </Card>
                      <Card as={"section"} className={cn("panel pr-diff")}>
                        <div className={cn("pr-commit-picker")}>
                          <Label>
                            {"Changes"}
                            <NativeSelect
                              aria-label={"Pull request commit"}
                              value={commit.value}
                              onValueChange={(next) => {
                                commit.value = next;
                              }}
                              onChange={() => loadFiles(1)}
                            >
                              <SelectOption value={""}>
                                {"All changes"}
                              </SelectOption>
                              {(() => {
                                const entries = commits.value;
                                return entries.map((item, index) => (
                                  <Fragment key={index}>
                                    <SelectOption value={item.sha}>
                                      {item.sha.slice(0, 8)}
                                      {" · "}
                                      {item.commit.message
                                        .split("\n")[0]
                                        .slice(0, 100)}
                                    </SelectOption>
                                  </Fragment>
                                ));
                              })()}
                            </NativeSelect>
                          </Label>
                          {commitsLimited.value ? (
                            <>
                              <small>
                                {
                                  "First 100 commits. Use GitHub for older or additional commits."
                                }
                              </small>
                            </>
                          ) : null}
                        </div>
                        <div className={cn("history-files-and-diff")}>
                          <section className={cn("history-files")}>
                            <div className={cn("panel-title")}>
                              {"Files · page "}
                              {filePage.value}
                            </div>
                            {files.value.length > 8 ? (
                              <>
                                <Input
                                  aria-label={"Filter pull request files"}
                                  placeholder={"Filter files"}
                                  value={fileQuery.value}
                                  onValueChange={(next) => {
                                    fileQuery.value = next;
                                  }}
                                />
                              </>
                            ) : null}
                            <div className={cn("file-list")}>
                              {(() => {
                                const entries = visibleFiles.value;
                                return entries.length === 0 ? (
                                  <>
                                    <p className={cn("empty-small")}>
                                      {filesLoading.value
                                        ? "Loading files…"
                                        : "No files on this page"}
                                    </p>
                                  </>
                                ) : (
                                  entries.map((file, index) => (
                                    <Fragment key={file.filename}>
                                      <Button
                                        onClick={() =>
                                          (selectedFile.value = file)
                                        }
                                        className={cn([
                                          "changed-file",
                                          {
                                            "file-selected":
                                              selectedFile.value?.filename ===
                                              file.filename,
                                          },
                                        ])}
                                      >
                                        <FileCode size={15} />
                                        <span>{file.filename}</span>
                                        <small className={cn("change-added")}>
                                          {"+"}
                                          {file.additions}
                                        </small>
                                        <small className={cn("change-deleted")}>
                                          {"−"}
                                          {file.deletions}
                                        </small>
                                      </Button>
                                    </Fragment>
                                  ))
                                );
                              })()}
                            </div>
                            <div className={cn("history-paging")}>
                              <Button
                                variant={"secondary"}
                                disabled={
                                  filesLoading.value || filePage.value === 1
                                }
                                onClick={() => loadFiles(filePage.value - 1)}
                              >
                                {"Previous"}
                              </Button>
                              <Button
                                variant={"secondary"}
                                disabled={
                                  filesLoading.value || !moreFiles.value
                                }
                                onClick={() => loadFiles(filePage.value + 1)}
                              >
                                {"Next"}
                              </Button>
                            </div>
                          </section>
                          <section className={cn("diff-panel")}>
                            <div className={cn("panel-title")}>
                              <span className={cn("diff-filename")}>
                                {selectedFile.value?.filename ??
                                  "Select a file"}
                              </span>
                            </div>
                            {filesLoading.value ? (
                              <>
                                <div className={cn("diff-empty")}>
                                  {"Loading diff…"}
                                </div>
                              </>
                            ) : (
                              <>
                                {selectedFile.value?.patch ? (
                                  <>
                                    <DiffView
                                      code={selectedFile.value.patch}
                                      label={"Pull request diff"}
                                    />
                                  </>
                                ) : (
                                  <>
                                    <div className={cn("diff-empty")}>
                                      {
                                        "No textual preview. The file may be binary or GitHub may have omitted its patch."
                                      }
                                    </div>
                                  </>
                                )}
                              </>
                            )}
                          </section>
                        </div>
                      </Card>
                      <Card as={"section"} className={cn("panel pr-review")}>
                        <h3>{"Submit a review"}</h3>
                        <div className={cn("inline-form")}>
                          <NativeSelect
                            aria-label={"Review action"}
                            value={reviewEvent.value}
                            disabled={busy.value}
                            onValueChange={(next) => {
                              reviewEvent.value = next;
                            }}
                          >
                            <SelectOption value={"COMMENT"}>
                              {"Comment"}
                            </SelectOption>
                            <SelectOption value={"APPROVE"}>
                              {"Approve"}
                            </SelectOption>
                            <SelectOption value={"REQUEST_CHANGES"}>
                              {"Request changes"}
                            </SelectOption>
                          </NativeSelect>
                        </div>
                        <Textarea
                          aria-label={"Review comment"}
                          placeholder={"Review comment"}
                          maxLength={20000}
                          rows={3}
                          value={reviewBody.value}
                          disabled={busy.value}
                          onValueChange={(next) => {
                            reviewBody.value = next;
                          }}
                        />
                        <Button
                          variant={"primary"}
                          disabled={
                            busy.value ||
                            detailLoading.value ||
                            detail.value.pull.state !== "open" ||
                            (reviewEvent.value !== "APPROVE" &&
                              !reviewBody.value.trim())
                          }
                          onClick={review}
                        >
                          {reviewEvent.value === "APPROVE"
                            ? "Approve pull request"
                            : reviewEvent.value === "REQUEST_CHANGES"
                              ? "Request changes"
                              : "Submit comment"}
                        </Button>
                      </Card>
                      <Card
                        as={"section"}
                        className={cn("panel pr-review-history")}
                      >
                        <h3>{"Reviews"}</h3>
                        {detail.value.reviews.length > 10 ? (
                          <>
                            <Input
                              aria-label={"Filter reviews"}
                              placeholder={"Filter reviews"}
                              value={reviewQuery.value}
                              onValueChange={(next) => {
                                reviewQuery.value = next;
                              }}
                            />
                          </>
                        ) : null}
                        {(() => {
                          const entries = reviews.value.slice(
                            reviewPage.value * 10,
                            reviewPage.value * 10 + 10,
                          );
                          return entries.length === 0 ? (
                            <>
                              <p>{"No reviews"}</p>
                            </>
                          ) : (
                            entries.map((review, index) => (
                              <Fragment key={index}>
                                <article>
                                  <strong>
                                    {"@"}
                                    {review.user.login}
                                  </strong>
                                  <span
                                    className={cn(
                                      review.state === "APPROVED" &&
                                        "change-added",
                                      review.state === "CHANGES_REQUESTED" &&
                                        "change-deleted",
                                    )}
                                  >
                                    {review.state.replaceAll("_", " ")}
                                  </span>
                                  {review.body ? (
                                    <>
                                      <pre>{review.body.slice(0, 20000)}</pre>
                                    </>
                                  ) : null}
                                </article>
                              </Fragment>
                            ))
                          );
                        })()}
                        {reviews.value.length > 10 ? (
                          <>
                            <div className={cn("history-paging")}>
                              <Button
                                variant={"secondary"}
                                disabled={reviewPage.value === 0}
                                onClick={() => reviewPage.value--}
                              >
                                {"Previous"}
                              </Button>
                              <span>{reviewPage.value + 1}</span>
                              <Button
                                variant={"secondary"}
                                disabled={
                                  (reviewPage.value + 1) * 10 >=
                                  reviews.value.length
                                }
                                onClick={() => reviewPage.value++}
                              >
                                {"Next"}
                              </Button>
                            </div>
                          </>
                        ) : null}
                        {!detail.value.reviews_complete ? (
                          <>
                            <p>
                              {
                                "Review history is incomplete; merge is blocked."
                              }
                            </p>
                          </>
                        ) : null}
                      </Card>
                      <Card as={"section"} className={cn("panel pr-merge")}>
                        <div>
                          <h3>{"Merge pull request"}</h3>
                          <p>
                            {canMerge.value
                              ? "GitHub policy and current checks allow a merge."
                              : "Merge is blocked until conflicts, checks, reviews and repository rules are satisfied."}
                          </p>
                        </div>
                        <NativeSelect
                          aria-label={"Merge method"}
                          value={method.value}
                          disabled={busy.value}
                          onValueChange={(next) => {
                            method.value = next;
                          }}
                        >
                          {(() => {
                            const entries = detail.value.merge_methods;
                            return entries.map((option, index) => (
                              <Fragment key={index}>
                                <SelectOption value={option}>
                                  {option === "squash"
                                    ? "Squash and merge"
                                    : option === "rebase"
                                      ? "Rebase and merge"
                                      : "Merge commit"}
                                </SelectOption>
                              </Fragment>
                            ));
                          })()}
                        </NativeSelect>
                        <Button
                          variant={"primary"}
                          disabled={!canMerge.value}
                          onClick={confirmMerge}
                        >
                          {"Merge…"}
                        </Button>
                      </Card>
                    </>
                  ) : (
                    <>
                      {detailLoading.value ? (
                        <>
                          <div role={"status"} className={cn("empty-block")}>
                            {"Loading pull request…"}
                          </div>
                        </>
                      ) : null}
                    </>
                  )}
                </div>
              </div>
            </>
          )}
        </section>
        {showCreate.value ? (
          <>
            <Modal
              canClose={!busy.value && !comparing.value}
              label={"Create pull request"}
              onclose={() => (showCreate.value = false)}
            >
              <h2>{"Create pull request"}</h2>
              <p>
                {context.value?.full_name}
                {" · From "}
                <strong>{git.value?.branch ?? "No branch selected"}</strong>
              </p>
              {account.value ? (
                <>
                  <p>
                    {"Creating as @"}
                    {account.value.login}
                  </p>
                </>
              ) : null}
              {error.value ? (
                <>
                  <p role={"alert"} className={cn("connection-error")}>
                    {error.value}
                  </p>
                </>
              ) : null}
              <Label htmlFor={"pr-base"}>{"Base branch"}</Label>
              <Input
                id={"pr-base"}
                list={"pr-base-options"}
                value={base.value}
                disabled={busy.value || comparing.value}
                onValueChange={(next) => {
                  base.value = next;
                }}
                onInput={() => {
                  comparison.value = null;
                  error.value = "";
                }}
              />
              <datalist id={"pr-base-options"}>
                {(() => {
                  const entries = context.value?.branches ?? [];
                  return entries.map((branch, index) => (
                    <Fragment key={index}>
                      <SelectOption value={branch} />
                    </Fragment>
                  ));
                })()}
              </datalist>
              {!git.value?.head ||
              git.value.dirty ||
              git.value.operation ||
              git.value.conflicts.length ||
              !git.value.upstream ||
              git.value.upstream_gone ||
              git.value.ahead ? (
                <>
                  <p>
                    {
                      "Commit and publish your latest changes before comparing branches."
                    }
                  </p>
                </>
              ) : (
                <>
                  {git.value.branch === base.value ? (
                    <>
                      <p>
                        {"Choose the branch that should receive these changes."}
                      </p>
                    </>
                  ) : null}
                </>
              )}
              {comparing.value ? (
                <>
                  <p role={"status"}>{"Comparing published changes…"}</p>
                </>
              ) : null}
              {compared ? (
                <>
                  <p role={"status"}>
                    {compared.ahead_by}{" "}
                    {compared.ahead_by === 1 ? "new commit" : "new commits"}
                    {" · "}
                    {compared.behind_by}
                    {" behind "}
                    {base.value}
                  </p>
                  {compared.files.length ? (
                    <>
                      <Collapsible
                        open={true}
                        heading={() => (
                          <>
                            {"Changed files "}
                            {compared.files.length === 300
                              ? "(first 300 from GitHub)"
                              : ""}
                          </>
                        )}
                      >
                        <ul className={cn("branch-results")}>
                          {(() => {
                            const entries = compared.files;
                            return entries.map((file, index) => (
                              <Fragment key={index}>
                                <li>
                                  {file.filename}
                                  {" · "}
                                  {file.status}
                                </li>
                              </Fragment>
                            ));
                          })()}
                        </ul>
                      </Collapsible>
                    </>
                  ) : null}
                  {compared.ahead_by ? (
                    <>
                      <Label htmlFor={"pr-title"}>{"Title"}</Label>
                      <Input
                        id={"pr-title"}
                        value={title.value}
                        maxLength={256}
                        disabled={busy.value || draftLoading.value}
                        onValueChange={(next) => {
                          title.value = next;
                        }}
                        onInput={persistDraft}
                      />
                      <Label htmlFor={"pr-body"}>{"Description"}</Label>
                      <Textarea
                        id={"pr-body"}
                        value={body.value}
                        rows={3}
                        maxLength={20000}
                        disabled={busy.value || draftLoading.value}
                        onValueChange={(next) => {
                          body.value = next;
                        }}
                        onInput={persistDraft}
                      />
                      <Label className={cn("scope-choice")}>
                        <Checkbox
                          checked={draft.value}
                          disabled={busy.value}
                          onCheckedChange={(next) => {
                            draft.value = next;
                          }}
                        />
                        {"Draft pull request"}
                      </Label>
                    </>
                  ) : (
                    <>
                      <p>
                        {"This branch has no new commits for "}
                        {base.value}
                        {". Choose another base."}
                      </p>
                    </>
                  )}
                </>
              ) : null}
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  disabled={busy.value || comparing.value}
                  onClick={() => (showCreate.value = false)}
                >
                  {"Cancel"}
                </Button>
                {title.value || body.value ? (
                  <>
                    <Button
                      disabled={
                        busy.value || comparing.value || draftLoading.value
                      }
                      onClick={discardDraft}
                      className={cn("text-action")}
                    >
                      {"Discard draft"}
                    </Button>
                  </>
                ) : null}
                {compared?.ahead_by ? (
                  <>
                    <Button
                      variant={"primary"}
                      disabled={!canCreate.value || !title.value.trim()}
                      onClick={create}
                    >
                      {"Create pull request"}
                    </Button>
                  </>
                ) : (
                  <>
                    <Button
                      variant={"primary"}
                      disabled={!canCompare.value}
                      onClick={compare}
                    >
                      {"Compare changes"}
                    </Button>
                  </>
                )}
              </div>
            </Modal>
          </>
        ) : null}
        {pendingMerge.value ? (
          <>
            <Modal
              canClose={!busy.value}
              label={"Confirm pull request merge"}
              onclose={() => {
                if (!busy.value) pendingMerge.value = null;
              }}
            >
              <h2>
                {"Merge #"}
                {pendingMerge.value.review.number}
                {" into "}
                {pendingMerge.value.review.base}
                {"?"}
              </h2>
              <p>{pendingMerge.value.title}</p>
              <p>
                {pendingMerge.value.review.repository}
                {" · @"}
                {pendingMerge.value.review.accountLogin}
              </p>
              <p>
                {"Method: "}
                {pendingMerge.value.method}
                {" · Head: "}
                {pendingMerge.value.review.head.slice(0, 12)}
              </p>
              <div className={cn("modal-actions")}>
                <Button
                  variant={"secondary"}
                  disabled={busy.value}
                  onClick={() => (pendingMerge.value = null)}
                >
                  {"Cancel"}
                </Button>
                <Button
                  variant={"primary"}
                  disabled={busy.value}
                  onClick={merge}
                >
                  {"Merge pull request"}
                </Button>
              </div>
            </Modal>
          </>
        ) : null}
      </>
    );
  };
}
export default observer(function PullRequests(props: Props) {
  return useController(createView, props);
});
