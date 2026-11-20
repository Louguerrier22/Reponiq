import { Check, ChevronsUpDown } from "lucide-react";
import { useState } from "react";
import { readablePath, type Repo } from "../../../lib/api";
import {
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from "../../shadcn/command";
import { Popover, PopoverContent, PopoverTrigger } from "../../shadcn/popover";
import { Button } from "../../ui";

export default function RepositoryPicker({
  id,
  repos,
  selectedId,
  disabled,
  onSelectedIdChange,
}: {
  id: string;
  repos: Repo[];
  selectedId: number | null;
  disabled?: boolean;
  onSelectedIdChange: (id: number | null) => void;
}) {
  const [open, setOpen] = useState(false);
  const selected = repos.find((repo) => repo.id === selectedId);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          variant="secondary"
          disabled={disabled}
          role="combobox"
          aria-expanded={open}
          className="repo-picker-trigger"
        >
          <span>{selected ? selected.name : "Choose a repository"}</span>
          <ChevronsUpDown size={16} />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="repo-picker-popover">
        <Command>
          <CommandInput
            placeholder="Search repositories"
            aria-label="Search repositories"
          />
          <CommandList>
            <CommandEmpty>No matching repositories.</CommandEmpty>
            {repos.map((repo) => (
              <CommandItem
                key={repo.id}
                value={String(repo.id)}
                keywords={[repo.name, readablePath(repo.path)]}
                onSelect={() => {
                  onSelectedIdChange(repo.id);
                  setOpen(false);
                }}
              >
                <div className="repo-picker-option">
                  <strong>{repo.name}</strong>
                  <small>{readablePath(repo.path)}</small>
                </div>
                {repo.id === selectedId && <Check size={15} />}
              </CommandItem>
            ))}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
