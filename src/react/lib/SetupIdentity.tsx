import { observer } from "mobx-react-lite";
import type { FormEvent } from "react";
import { api, type Repo } from "../../lib/api";
import { derived, state, useController, type Lifecycle } from "../controller";
import { Button, Field, Input } from "../ui";
import RepositoryPicker from "./ui/RepositoryPicker";
import { cn } from "../utils";
type BaseProps = {
  repos: Repo[];
  busy?: boolean;
};
type Props = BaseProps & {
  onBusyChange?: (value: NonNullable<BaseProps["busy"]>) => void;
};
function createView(lifecycle: Lifecycle<Props>) {
  const repos = derived(() => lifecycle.props().repos);
  const busy = lifecycle.bound("busy", false, "onBusyChange");
  let id = state<number | null>(null);
  let name = state("");
  let email = state("");
  let error = state("");
  let saved = state("");
  const repo = derived(() => repos.value.find((row) => row.id === id.value));
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!repo.value || !name.value.trim() || !email.value.trim() || busy.value)
      return;
    const { id, path, registration } = repo.value;
    busy.value = true;
    error.value = "";
    saved.value = "";
    try {
      await api.setIdentity(
        { id, path, registration },
        { name: name.value.trim(), email: email.value.trim() },
      );
      saved.value = "Git identity saved for " + repo.value.name;
    } catch (reason) {
      error.value = String(reason);
    } finally {
      busy.value = false;
    }
  }
  return () => (
    <>
      <form onSubmit={save} className={cn("setup-identity")}>
        <Field id={"setup-identity-repo"} label={"Repository"}>
          <RepositoryPicker
            id={"setup-identity-repo"}
            repos={repos.value}
            disabled={busy.value}
            selectedId={(() => id.value)()}
            onSelectedIdChange={(value) => {
              if (value !== id.value) {
                id.value = value;
                name.value = "";
                email.value = "";
                error.value = "";
                saved.value = "";
              }
            }}
          />
        </Field>
        <Field id={"setup-git-name"} label={"Commit name"}>
          <Input
            id={"setup-git-name"}
            value={name.value}
            disabled={busy.value}
            required={true}
            onValueChange={(next) => {
              name.value = next;
            }}
          />
        </Field>
        <Field id={"setup-git-email"} label={"Commit email"}>
          <Input
            id={"setup-git-email"}
            type={"email"}
            value={email.value}
            disabled={busy.value}
            required={true}
            onValueChange={(next) => {
              email.value = next;
            }}
          />
        </Field>
        {error.value ? (
          <>
            <p role={"alert"} className={cn("connection-error")}>
              {error.value}
            </p>
          </>
        ) : null}
        {saved.value ? (
          <>
            <p role={"status"}>{saved.value}</p>
          </>
        ) : null}
        <Button
          type={"submit"}
          variant={"secondary"}
          disabled={
            busy.value ||
            !repo.value ||
            !name.value.trim() ||
            !email.value.trim()
          }
        >
          {"Save Git identity"}
        </Button>
      </form>
    </>
  );
}
export default observer(function SetupIdentity(props: Props) {
  return useController(createView, props);
});
