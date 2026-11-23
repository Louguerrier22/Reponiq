import { FolderGit2, GitBranch } from "lucide-react";
import bitbucket from "../../../assets/platforms/bitbucket.svg";
import codeberg from "../../../assets/platforms/codeberg.svg";
import gitea from "../../../assets/platforms/gitea.svg";
import github from "../../../assets/platforms/github.svg";
import { remoteInfo, type Platform } from "../../lib/platform";
import { ThemeIcon } from "../icons";

const logos: Partial<Record<Platform, string>> = { github, bitbucket, codeberg, gitea };

export default function PlatformIcon({ remote, size = 24 }: {
  remote: string | null;
  size?: number;
}) {
  const { platform, label } = remoteInfo(remote);
  const logo = logos[platform];
  const Fallback = platform === "local" ? FolderGit2 : GitBranch;

  return (
    <span className="platform-icon" role="img" aria-label={label} title={label}>
      {logo ? <ThemeIcon src={logo} size={size} /> : <Fallback size={size} aria-hidden="true" />}
    </span>
  );
}
