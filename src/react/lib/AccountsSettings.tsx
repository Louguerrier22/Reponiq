import { Plus, ShieldCheck } from "lucide-react";
import { useId, useState } from "react";
import type { Account } from "../../lib/api";
import { platformLabels } from "../../lib/platform";
import { NativeSelectOption } from "../shadcn/native-select";
import { Button, Card, Field, NativeSelect } from "../ui";
import PlatformIcon from "./PlatformIcon";

const providers = Object.entries(platformLabels).filter(([id]) => id !== "local");

type Props = {
  accounts: Account[];
  working: boolean;
  onConnect: () => void;
  onSync: (account: Account) => void;
  onDisconnect: (account: Account) => void;
};

export default function AccountsSettings({
  accounts, working, onConnect, onSync, onDisconnect,
}: Props) {
  const providerId = useId();
  const accountId = useId();
  const [provider, setProvider] = useState("github");
  const [selectedId, setSelectedId] = useState("");
  const selected = accounts.find((account) => String(account.id) === selectedId) ?? accounts[0];

  return (
    <>
      <Card className="panel">
        <div className="panel-title">
          Accounts
          {provider === "github" && <span>{accounts.length} connected</span>}
        </div>
        <div className="account-fields">
          <Field id={providerId} label="Provider">
            <NativeSelect id={providerId} value={provider} onValueChange={setProvider}>
              {providers.map(([id, label]) => (
                <NativeSelectOption key={id} value={id}>
                  {id === "git" ? "Other Git hosts" : label}
                </NativeSelectOption>
              ))}
            </NativeSelect>
          </Field>
          {provider === "github" && accounts.length > 1 && (
            <Field id={accountId} label="Connected account">
              <NativeSelect id={accountId} value={String(selected?.id ?? "")} onValueChange={setSelectedId}>
                {accounts.map((account) => (
                  <NativeSelectOption key={account.id} value={String(account.id)}>
                    @{account.login}
                  </NativeSelectOption>
                ))}
              </NativeSelect>
            </Field>
          )}
        </div>
        {provider === "github" ? (
          <>
            {selected ? (
              <div className="account-row">
                <div className="account-avatar">
                  <PlatformIcon remote="https://github.com" size={19} />
                </div>
                <div className="account-info">
                  <strong>@{selected.login}</strong>
                  <small>{selected.api_host}</small>
                </div>
                <Button className="micro" disabled={working} onClick={() => onSync(selected)}>
                  Sync workflows
                </Button>
                <Button className="micro dangerous" disabled={working} onClick={() => onDisconnect(selected)}>
                  Disconnect
                </Button>
              </div>
            ) : (
              <div className="empty-small">No GitHub accounts connected.</div>
            )}
            <div className="panel-footer">
              <Button data-tour="connect" variant="primary" disabled={working} onClick={onConnect}>
                <Plus size={15} />
                {selected ? "Connect another account" : "Connect GitHub"}
              </Button>
            </div>
          </>
        ) : (
          <p className="account-git-note">Clone by URL. Git uses your credential helper or SSH key for this host.</p>
        )}
      </Card>
      {provider === "github" && (
        <Card className="panel info-panel">
          <ShieldCheck size={21} />
          <div>
            <strong>Credentials stay on your device</strong>
            <p>GitHub access tokens are stored in your operating system’s credential manager.</p>
          </div>
        </Card>
      )}
    </>
  );
}
