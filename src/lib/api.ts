import { invoke } from '@tauri-apps/api/core';
import { remoteInfo } from './platform';
export type Repo = { id: number; path: string; name: string; remote_url: string | null; pinned: boolean; collection: string | null; tags: string[]; branch: string; dirty: number; ahead: number; behind: number; github_full_name: string | null; ci_conclusion: string | null; ci_status: string | null; status_error?: string | null;status_pending?:boolean };
export type RepoSummary = Pick<Repo,'id'|'branch'|'dirty'|'ahead'|'behind'|'status_error'>;
export type Commit = { hash: string; subject: string; author: string; timestamp: number };
export type Activity = { id: number; repo_id: number | null; repo_name: string | null; kind: string; title: string; detail: string | null; occurred_at: number };
export type Account = { id: number; login: string; api_host: string; created_at: number };
export type FileChange = { x: string; y: string; path: string; original_path: string | null };
export type GitState = { has_head:boolean;head:string|null;branch:string;upstream:string|null;upstream_gone:boolean;remote:string|null;remote_ref:string|null;remotes:string[];operation:string|null;conflicts:string[];staged:number;dirty:number;ahead:number;behind:number };
export type RepoDetails = { commits: Commit[]; changes: FileChange[]; branches: string[]; git:GitState };
export type CommitDetails = { parents:string[];body:string;files:{path:string;status:string}[];truncated:boolean };
export type ConflictDetails = {path:string;ours:string|null;theirs:string|null;current:string};
export type Pull = {number:number;title:string;body:string|null;state:string;draft:boolean;merged:boolean|null;mergeable:boolean|null;mergeable_state:string|null;user:{login:string};head:{label:string;sha:string;ref:string};base:{label:string;sha:string;ref:string}};
export type Review = {id:number;user:{login:string};state:string;body:string|null;commit_id:string|null};
export type PullFile = {filename:string;status:string;additions:number;deletions:number;patch:string|null;previous_filename:string|null};
export type PullDetails = {pull:Pull;reviews:Review[];reviews_complete:boolean;checks:{runs:{name:string;status:string;conclusion:string|null}[];legacy_state:string;complete:boolean;ready:boolean};merge_methods:string[]};
export type PullCommit = {sha:string;commit:{message:string}};
export type PullContext = {full_name:string;default_branch:string;branches:string[];merge_methods:string[]};
export type Page<T> = {items:T[];has_more:boolean};
export type LoginCode = { id: number; user_code: string; expires_in: number; interval: number };
export type LoginStatus = { status: 'pending'; retry_after: number; message: string | null } | { status: 'connected'; account: Account };
export type OnlineRepo = {id:number;name:string;full_name:string;private:boolean};
export type RepositoryOwner = {login:string;type:string};
export type CreatedRepo = {repo:Repo;warning:string|null};
export type WorkspaceSettings = {default_folder:string|null;auto_detect:boolean;setup_complete:boolean};
export type DiscoveredRepo = {name:string;path:string;source:string};
export type Discovery = {items:DiscoveredRepo[];limited:boolean;notes:string[]};
export type ImportSource = 'default_folder'|'folder'|'github_desktop'|'vs_code';
export const api = {
  walkthroughSeen: () => invoke<boolean>('walkthrough_seen'),
  finishWalkthrough: () => invoke<void>('finish_walkthrough'),
  onlineRepos: (accountId:number,page=1) => invoke<Page<OnlineRepo>>('list_online_repos',{accountId,page}),
  repositoryOwners: (accountId:number,page=1) => invoke<Page<RepositoryOwner> & {warning:string|null}>('repository_owners',{accountId,page}),
  cloneRepository: (accountId:number,repositoryId:number,parent:string,name:string) => invoke<Repo>('clone_repository',{accountId,repositoryId,parent,name}),
  createRepository: (input:{parent:string;name:string;account_id:number|null;owner:string|null;private:boolean}) => invoke<CreatedRepo>('create_repository',{input}),
  workspaceSettings: () => invoke<WorkspaceSettings>('workspace_settings'),
  saveWorkspaceSettings: (input:WorkspaceSettings) => invoke<WorkspaceSettings>('save_workspace_settings',{input}),
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
  pullDetails: (id:number,accountId:number,number:number) => invoke<PullDetails>('pull_request_details',{id,accountId,number}),
  pullFiles: (id:number,accountId:number,number:number,page:number,commit:string|null) => invoke<Page<PullFile>>('pull_request_files',{id,accountId,number,page,commit}),
  pullCommits: (id:number,accountId:number,number:number) => invoke<Page<PullCommit>>('pull_request_commits',{id,accountId,number}),
  createPull: (id:number,accountId:number,input:{title:string;body:string;base:string;head:string;expected_head:string;draft:boolean}) => invoke<Pull>('create_pull_request',{id,accountId,input}),
  reviewPull: (id:number,accountId:number,number:number,head:string,event:string,body:string) => invoke<Review>('review_pull_request',{id,accountId,number,head,event,body}),
  mergePull: (id:number,accountId:number,number:number,head:string,method:string) => invoke<string>('merge_pull_request',{id,accountId,number,head,method}),
  git: (id: number, action: string, args: string[] = [], expected: {branch:string;head:string|null}|null = null) => invoke<string>('git_action', { id, action, args, expected }),
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
