import { observer } from "mobx-react-lite";
import {
api,
readablePath,
type AppPreferences,
type RepoContext,
type RepoPreferences,
type Target,
} from "../../lib/api";
import { confirm } from "../confirmation";
import {
SelectOption,
derived,
state,
untrack,
useController,
type Lifecycle
} from "../controller";
import { Button,Checkbox,Collapsible,Input,Label,NativeSelect,Textarea } from "../ui";
import { cn } from "../utils";
type Props = {
  target?: Target | null;
  app: AppPreferences;
  preferences?: RepoPreferences | null;
  context?: RepoContext | null;
  onsaved: (value: AppPreferences | RepoPreferences) => void;
  onidentity?: (value: RepoContext) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const target = derived(() => lifecycle.props().target ?? null);
  const app = derived(() => lifecycle.props().app);
  const preferences = derived(() => lifecycle.props().preferences ?? null);
  const context = derived(() => lifecycle.props().context ?? null);
  const onsaved = derived(() => lifecycle.props().onsaved);
  const onidentity = derived(() => lifecycle.props().onidentity ?? (() => {}));
  const defaults: RepoPreferences = {
    fetch_mode: "inherit",
    warn_default: "inherit",
    editor: null,
    review_paths: [],
    preferred_base: null,
  };
  let global = state<AppPreferences>(untrack(() => ({ ...app.value })));
  let local = state<RepoPreferences>(
    untrack(() => ({ ...defaults, ...preferences.value })),
  );
  let paths = state(untrack(() => local.value.review_paths.join("\n")));
  let author = state(untrack(() => context.value?.local_name ?? ""));
  let email = state(untrack(() => context.value?.local_email ?? ""));
  let busy = state(false),
    message = state(""),
    error = state("");
  let executable = derived(() =>
    target.value ? local.value.editor : global.value.editor,
  );
  lifecycle.effect(() => {
    if (context.value) {
      author.value = context.value.local_name ?? "";
      email.value = context.value.local_email ?? "";
    }
  });
  async function browse() {
    if (busy.value) return;
    busy.value = true;
    error.value = "";
    try {
      const path = await api.pickEditor();
      if (path) {
        if (target.value) local.value.editor = path;
        else global.value.editor = path;
      }
    } catch (reason) {
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  async function save() {
    if (busy.value) return;
    if (
      !target.value &&
      app.value.save_drafts &&
      !global.value.save_drafts &&
      !(await confirm({
        title: "Delete saved drafts on this device?",
        description: "Open drafts stay available until you close Reponiq.",
        action: "Delete saved drafts",
        destructive: true,
      }))
    )
      return;
    busy.value = true;
    error.value = "";
    message.value = "";
    try {
      if (target.value) {
        const saved = await api.saveRepoWorkflow(target.value, {
          ...local.value,
          preferred_base: local.value.preferred_base?.trim() || null,
          review_paths: [
            ...new Set(
              paths.value.split(/\r?\n/).filter((path) => path.trim()),
            ),
          ],
        });
        local.value = saved;
        onsaved.value(saved);
      } else {
        global.value = await api.saveWorkflowApp({ ...global.value });
        onsaved.value(global.value);
      }
      message.value = "Preferences saved.";
    } catch (reason) {
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  async function saveIdentity(useDefault: boolean) {
    if (!target.value || busy.value) return;
    busy.value = true;
    error.value = "";
    message.value = "";
    try {
      const next = await api.setIdentity(
        target.value,
        useDefault
          ? null
          : { name: author.value.trim(), email: email.value.trim() },
      );
      author.value = next.local_name ?? "";
      email.value = next.local_email ?? "";
      onidentity.value(next);
      message.value = next.author
        ? "Git author is " + next.author
        : "Git has no author configured.";
    } catch (reason) {
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  return () => (
    <>
      <div className={cn("workflow-settings-form")}>
        {target.value ? (
          <>
            <Label>
              {"Fetch on launch "}
              <NativeSelect
                value={local.value.fetch_mode}
                disabled={busy.value}
                onValueChange={(next) => {
                  local.value.fetch_mode = next;
                }}
              >
                <SelectOption value={"inherit"}>
                  {"Follow startup scope"}
                </SelectOption>
                <SelectOption value={"on"}>{"Include at launch"}</SelectOption>
                <SelectOption value={"off"}>{"Manual only"}</SelectOption>
              </NativeSelect>
            </Label>
            <Label>
              {"Default-branch reminder "}
              <NativeSelect
                value={local.value.warn_default}
                disabled={busy.value}
                onValueChange={(next) => {
                  local.value.warn_default = next;
                }}
              >
                <SelectOption value={"inherit"}>
                  {"Use app default"}
                </SelectOption>
                <SelectOption value={"on"}>{"Always warn"}</SelectOption>
                <SelectOption value={"off"}>{"Off"}</SelectOption>
              </NativeSelect>
            </Label>
            <Label>
              {"Preferred PR base"}
              <Input
                value={local.value.preferred_base}
                placeholder={"Use repository default"}
                maxLength={200}
                disabled={busy.value}
                onValueChange={(next) => {
                  local.value.preferred_base = next;
                }}
              />
            </Label>
            <Label>
              {"Collapse review paths"}
              <Textarea
                value={paths.value}
                placeholder={
                  "One file or folder per line\npackage-lock.json\ngenerated/"
                }
                rows={3}
                disabled={busy.value}
                onValueChange={(next) => {
                  paths.value = next;
                }}
              />
            </Label>
          </>
        ) : (
          <>
            <Label className={cn("scope-choice")}>
              <Checkbox
                checked={global.value.save_drafts}
                disabled={busy.value}
                onCheckedChange={(next) => {
                  global.value.save_drafts = next;
                }}
              />
              {" Save drafts on this device"}
            </Label>
            <Label className={cn("scope-choice")}>
              <Checkbox
                checked={global.value.warn_default}
                disabled={busy.value}
                onCheckedChange={(next) => {
                  global.value.warn_default = next;
                }}
              />
              {" Remind me before committing to the default branch"}
            </Label>
          </>
        )}
        <Label>
          {target.value ? "Repository editor" : "Default editor"}
          <Input
            value={executable.value ? readablePath(executable.value) : ""}
            placeholder={
              target.value ? "Use app default" : "Choose an installed editor"
            }
            readOnly={true}
          />
        </Label>
        <div className={cn("toolbar")}>
          <Button variant={"secondary"} disabled={busy.value} onClick={browse}>
            {"Choose editor…"}
          </Button>
          {executable.value ? (
            <>
              <Button
                disabled={busy.value}
                onClick={() => {
                  if (target.value) local.value.editor = null;
                  else global.value.editor = null;
                }}
                className={cn("text-action")}
              >
                {target.value ? "Use app default" : "Clear editor"}
              </Button>
            </>
          ) : null}
          <Button variant={"primary"} disabled={busy.value} onClick={save}>
            {"Save preferences"}
          </Button>
        </div>
        {target.value && context.value ? (
          <>
            <Collapsible
              open={!context.value.author}
              className={cn("workflow-identity")}
              heading={() => <>{"Git commit author"}</>}
            >
              <p>{context.value.author ?? "Git has no author configured."}</p>
              <Label>
                {"Name"}
                <Input
                  value={author.value}
                  disabled={busy.value}
                  maxLength={200}
                  onValueChange={(next) => {
                    author.value = next;
                  }}
                />
              </Label>
              <Label>
                {"Email"}
                <Input
                  type={"email"}
                  value={email.value}
                  disabled={busy.value}
                  maxLength={254}
                  onValueChange={(next) => {
                    email.value = next;
                  }}
                />
              </Label>
              <div className={cn("toolbar")}>
                <Button
                  variant={"secondary"}
                  disabled={
                    busy.value || !author.value.trim() || !email.value.trim()
                  }
                  onClick={() => saveIdentity(false)}
                >
                  {"Set repository author"}
                </Button>
                <Button
                  disabled={busy.value}
                  onClick={() => saveIdentity(true)}
                  className={cn("text-action")}
                >
                  {"Use Git default"}
                </Button>
              </div>
            </Collapsible>
          </>
        ) : null}
        {message.value ? (
          <>
            <p role={"status"} className={cn("inline-success")}>
              {message.value}
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
      </div>
    </>
  );
}
export default observer(function WorkflowSettings(props: Props) {
  return useController(createView, props);
});
