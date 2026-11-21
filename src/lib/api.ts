import { invoke } from './diagnostics';
import { remoteInfo } from './platform';
import type { AppearanceSettings, RegistryTheme, SetupStep, Theme } from './appearance';
export type Repo = { id: number; registration?:string|null; path: string; name: string; remote_url: string | null; pinned: boolean; collection: string | null; tags: string[]; branch: string; dirty: number; ahead: number; behind: number; github_full_name: string | null; ci_conclusion: string | null; ci_status: string | null; status_error?: string | null;status_pending?:boolean; conflicts?:number; missing?:boolean };
export type RepoSummary = Pick<Repo,'id'|'path'|'registration'|'branch'|'dirty'|'ahead'|'behind'|'status_error'|'conflicts'|'missing'>;
export type Target = Pick<Repo,'id'|'path'|'registration'>;
export type Override = 'inherit'|'on'|'off';
export type RepoPreferences = {fetch_mode:Override;warn_default:Override;editor:string|null;review_paths:string[];preferred_base:string|null};
export type RepoWorkflow = RepoPreferences & Target & {pr_login:string|null;pin_position:number|null;opened_at:number|null;fetched_at:number|null;main_worktree:string|null;cached_status:Pick<Repo,'branch'|'dirty'|'ahead'|'behind'|'conflicts'>|null};
export type AppPreferences = {save_drafts:boolean;warn_default:boolean;editor:string|null};
export type RepoContext = {default_branch:string|null;author:string|null;local_name:string|null;local_email:string|null};
export type Location = {path:string;branch:string;remote:string|null;matching:'match'|'unknown'|'mismatch'};
export type DraftScope = {branch:string;account:string;base:string};
export type Draft = {title:string;body:string};
export type GitExpectation = {branch:string;head:string|null;path?:string;author?:string;registration?:string|null};
export class GitFailure extends Error {
  constructor(message:string, readonly code:string) { super(message); }
}
export type Commit = { hash: string; subject: string; author: string; timestamp: number };
export type Activity = { id: number; repo_id: number | null; repo_name: string | null; kind: string; title: string; detail: string | null; occurred_at: number };
export type Account = { id: number; login: string; api_host: string; created_at: number };
export type FileChange = { x: string; y: string; path: string; original_path: string | null };
export type GitState = { has_head:boolean;head:string|null;branch:string;upstream:string|null;upstream_gone:boolean;remote:string|null;remote_ref:string|null;remotes:string[];operation:string|null;conflicts:string[];staged:number;dirty:number;ahead:number;behind:number };
export type Branch = {reference:string;name:string;remote:string|null;in_worktree?:boolean};
export type RepoDetails = { commits: Commit[]; changes: FileChange[]; branches: Branch[]; saved_changes:{oid:string;timestamp:number}[]; git:GitState };
export type CommitDetails = { parents:string[];body:string;files:{path:string;status:string}[];truncated:boolean };
export type ConflictDetails = {path:string;ours:string|null;theirs:string|null;current:string};
export type Pull = {number:number;title:string;body:string|null;state:string;draft:boolean;merged:boolean|null;mergeable:boolean|null;mergeable_state:string|null;user:{login:string};head:{label:string;sha:string;ref:string};base:{label:string;sha:string;ref:string}};
export type Review = {id:number;user:{login:string};state:string;body:string|null;commit_id:string|null};
export type PullFile = {filename:string;status:string;additions:number;deletions:number;patch:string|null;previous_filename:string|null};
export type PullDetails = {repository:string;repository_id:string;pull:Pull;reviews:Review[];reviews_complete:boolean;checks:{runs:{name:string;status:string;conclusion:string|null;web_url?:string|null}[];legacy_state:string;complete:boolean;ready:boolean};merge_methods:string[]};
export type PullMergeScope = {target:Target;accountId:number;accountLogin:string};
export type ReviewedPull = PullMergeScope & {repository:string;repositoryId:string;number:number;head:string;base:string};
export type PullCommit = {sha:string;commit:{message:string}};
export type PullContext = {full_name:string;default_branch:string;branches:string[];merge_methods:string[]};
export type PullComparison = {base_commit:{sha:string};ahead_by:number;behind_by:number;files:{filename:string;status:string}[]};
export type Page<T> = {items:T[];has_more:boolean};
export type LoginCode = { id: number; user_code: string; expires_in: number; interval: number };
export type LoginStatus = { status: 'pending'; retry_after: number; message: string | null } | { status: 'connected'; account: Account };
export type OnlineRepo = {id:number;name:string;full_name:string;private:boolean};
export type RepositoryOwner = {login:string;type:string};
export type CreatedRepo = {repo:Repo;warning:string|null};
export type WorkspaceSettings = {default_folder:string|null;auto_detect:boolean;setup_complete:boolean};
export type FetchSettings = {enabled:boolean;scope:'pinned'|'all'|'selected'};
export type DiscoveredRepo = {name:string;path:string;source:string};
export type Discovery = {items:DiscoveredRepo[];limited:boolean;notes:string[]};
export type ImportSource = 'default_folder'|'folder'|'github_desktop'|'vs_code';
export const api = {
  appearance: () => invoke<AppearanceSettings>('appearance_settings'),
  saveAppearance: (input: Theme) => invoke<Theme>('save_appearance', { input }),
  previewRegistryTheme: (url: string) => invoke<RegistryTheme>('preview_registry_theme', { url }),
  advanceSetup: (expected: SetupStep) => invoke<SetupStep>('advance_setup', { expected }),
  walkthroughSeen: () => invoke<boolean>('walkthrough_seen'),
  finishWalkthrough: () => invoke<void>('finish_walkthrough'),
  onlineRepos: (accountId:number,page=1) => invoke<Page<OnlineRepo>>('list_online_repos',{accountId,page}),
  repositoryOwners: (accountId:number,page=1) => invoke<Page<RepositoryOwner> & {warning:string|null}>('repository_owners',{accountId,page}),
  cloneRepository: (accountId:number,repositoryId:number,parent:string,name:string) => invoke<Repo>('clone_repository',{accountId,repositoryId,parent,name}),
  cloneRepositoryUrl: (url:string,parent:string,name:string) => invoke<Repo>('clone_repository_url',{url,parent,name}),
  createRepository: (input:{parent:string;name:string;account_id:number|null;owner:string|null;private:boolean}) => invoke<CreatedRepo>('create_repository',{input}),
  workspaceSettings: () => invoke<WorkspaceSettings>('workspace_settings'),
  saveWorkspaceSettings: (input:WorkspaceSettings) => invoke<WorkspaceSettings>('save_workspace_settings',{input}),
  fetchSettings: () => invoke<FetchSettings>('fetch_settings'),
  saveFetchSettings: (input:FetchSettings, changes: {target: Target; mode: Override}[] = []) =>
    invoke<FetchSettings>('save_fetch_settings',{input, changes}),
  saveRepoFetchMode: (target: Target, mode: Override) =>
    invoke<void>('save_repo_fetch_mode',{target, mode}),
  launchFetch: (id:number,batch:number) => invoke<boolean>('launch_fetch',{id,batch}),
  workflowApp: () => invoke<AppPreferences>('workflow_app'),
  saveWorkflowApp: (input:AppPreferences) => invoke<AppPreferences>('save_workflow_app',{input}),
  repoWorkflow: () => invoke<RepoWorkflow[]>('list_repo_workflow'),
  saveRepoWorkflow: (target:Target,input:RepoPreferences) => invoke<RepoPreferences>('save_repo_workflow',{target,input}),
  visitRepo: (target:Target,branch:string|null) => invoke<string[]>('visit_repo',{target,branch}),
  movePin: (target:Target,earlier:boolean) => invoke<void>('move_pin',{target,earlier}),
  readDraft: (target:Target,scope:DraftScope) => invoke<Draft|null>('read_workflow_draft',{target,scope}),
  saveDraft: (target:Target,scope:DraftScope,input:Draft|null) => invoke<void>('save_workflow_draft',{target,scope,input}),
  repoContext: (target:Target) => invoke<RepoContext>('repo_context',{target}),
  setIdentity: (target:Target,input:{name:string;email:string}|null) => invoke<RepoContext>('set_git_identity',{target,input}),
  openRepoLocation: (target:Target,action:'editor'|'terminal'|'folder'|'worktree',reference:string|null=null) => invoke<void>('open_repo_location',{target,action,reference}),
  pickEditor: () => invoke<string|null>('pick_editor'),
  inspectLocation: (target:Target,path:string|null) => invoke<Location>('inspect_repo_location',{target,path}),
  relinkRepo: (target:Target,path:string,confirmUnknown:boolean) => invoke<Location>('relink_repo',{target,path,confirmUnknown}),
  undoCommit: (target:Target,expected:{branch:string;head:string}) => invoke<string>('undo_local_commit',{target,expected}),
  openCheck: (repoId:number,number:number,webUrl:string|null) => invoke<void>('open_pull_check',{id:repoId,number,webUrl}),
  autostartStatus: () => invoke<boolean>('autostart_status'),
  setAutostart: (enabled:boolean) => invoke<boolean>('set_autostart',{enabled}),
  discover: (source:ImportSource,folder:string|null=null) => invoke<Discovery>('discover_repositories',{source,folder}),
  importRepositories: (paths:string[]) => invoke<{added:number;remaining:string[];failures?:{path:string;message:string}[]}>('import_repositories',{paths}),
  summary: (id:number) => invoke<RepoSummary>('repo_summary',{id}),
  repos: () => invoke<Repo[]>('list_repos'),
  add: (path: string) => invoke<Repo>('add_repo', { path }),
  pickRepoFolder: (purpose:'repository'|'parent'='repository') => invoke<string | null>('pick_repo_folder',{purpose}),
  remove: (id: number) => invoke<void>('remove_repo', { id }),
  pin: (id: number, pinned: boolean) => invoke<void>('pin_repo', { id, pinned }),
  organize: (id: number, collection: string, tags: string[]) => invoke<void>('organize_repo', { id, collection, tags }),
  refresh: () => invoke<Repo[]>('refresh_repos'),
  details: (id: number) => invoke<RepoDetails>('repo_details', { id }),
  history: (id:number,skip:number) => invoke<Commit[]>('history_page',{id,skip}),
  commitDetails: (id:number,hash:string) => invoke<CommitDetails>('commit_details',{id,hash}),
  commitDiff: (id:number,hash:string,file:string|null) => invoke<string>('commit_diff',{id,hash,file}),
  conflict: (id:number,file:string) => invoke<ConflictDetails>('conflict_details',{id,file}),
  pullContext: (id:number,accountId:number) => invoke<PullContext>('pull_context',{id,accountId}),
  pulls: (id:number,accountId:number,page:number) => invoke<Page<Pull>>('list_pull_requests',{id,accountId,page}),
  pullDetails: (scope:PullMergeScope,number:number) => invoke<PullDetails>('pull_request_details',{...scope,number}),
  pullFiles: (review:ReviewedPull,page:number,commit:string|null) => invoke<Page<PullFile>>('pull_request_files',{review,page,commit}),
  pullCommits: (review:ReviewedPull) => invoke<Page<PullCommit>>('pull_request_commits',{review}),
  createPull: (id:number,accountId:number,input:{title:string;body:string;base:string;head:string;expected_head:string;expected_base:string;draft:boolean}) => invoke<{pull:Pull;matches_comparison:boolean}>('create_pull_request',{id,accountId,input}),
  comparePull: (id:number,accountId:number,base:string,head:string,expectedHead:string) => invoke<PullComparison>('compare_pull_branches',{id,accountId,base,head,expectedHead}),
  reviewPull: (id:number,accountId:number,number:number,head:string,event:string,body:string) => invoke<Review>('review_pull_request',{id,accountId,number,head,event,body}),
  mergePull: (review:ReviewedPull,method:string) => invoke<string>('merge_pull_request',{review,method}),
  git: async (id: number, action: string, args: string[] = [], expected: GitExpectation|null = null) => {
    try { return await invoke<string>('git_action', {id, action, args, expected}); }
    catch (error) {
      if (error && typeof error==='object' && 'message' in error && 'code' in error) {
        throw new GitFailure(String(error.message),String(error.code));
      }
      throw error;
    }
  },
  activity: () => invoke<Activity[]>('get_activity'),
  accounts: () => invoke<Account[]>('list_accounts'),
  connect: (token: string, apiHost = 'https://api.github.com') => invoke<Account>('connect_account', { token, apiHost }),
  browserLoginAvailable: () => invoke<boolean>('browser_login_available'),
  startLogin: (privateRepos: boolean) => invoke<LoginCode>('start_github_login', { privateRepos }),
  openLogin: (id: number) => invoke<void>('open_github_login', { id }),
  pollLogin: (id: number) => invoke<LoginStatus>('poll_github_login', { id }),
  cancelLogin: (id: number) => invoke<void>('cancel_github_login', { id }),
  disconnect: (id: number) => invoke<void>('disconnect_account', { id }),
  sync: (id: number) => invoke<string>('sync_account', { id }),
};
export function filteredRepos(repos: Repo[], query: string, view: string) {
  const q = query.trim().toLocaleLowerCase();
  return repos.filter(r => (view !== 'pinned' || r.pinned) && (view !== 'attention' || !!r.status_error || r.dirty > 0 || r.behind > 0 || r.ci_conclusion === 'failure') && (!view.startsWith('collection:') || r.collection === view.slice(11)) && (!q || [r.name, r.path, r.collection ?? '', r.branch, r.github_full_name ?? '', remoteInfo(r.remote_url).label, remoteInfo(r.remote_url).host, remoteInfo(r.remote_url).project, ...r.tags].some(v => v.toLocaleLowerCase().includes(q)))).sort((a,b) => Number(b.pinned)-Number(a.pinned) || a.name.localeCompare(b.name));
}

// Hide Windows canonical-path syntax in display fields; native commands still validate paths.
export function readablePath(path:string){return path.startsWith("\\\\?\\")&&!path.startsWith("\\\\?\\UNC\\")?path.slice(4):path;}
