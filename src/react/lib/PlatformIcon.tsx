import { FolderGit2,GitBranch } from "lucide-react";
import { observer } from "mobx-react-lite";
import bitbucket from "../../../assets/platforms/bitbucket.svg";
import codeberg from "../../../assets/platforms/codeberg.svg";
import gitea from "../../../assets/platforms/gitea.svg";
import github from "../../../assets/platforms/github.svg";
import { remoteInfo } from "../../lib/platform";
import {
derived,
useController,
type Lifecycle
} from "../controller";
import { cn } from "../utils";
type Props = {
  remote: string | null;
  size?: number;
};
function createView(lifecycle: Lifecycle<Props>) {
  const remote = derived(() => lifecycle.props().remote);
  const size = derived(() => lifecycle.props().size ?? 24);
  const info = derived(() => remoteInfo(remote.value));
  const logos: Record<string, string> = { github, bitbucket, codeberg, gitea };
  return () => (
    <>
      <span
        title={info.value.label}
        className={cn("platform-icon")}
        style={{
          width: String(size.value) + "px",
          height: String(size.value) + "px",
        }}
      >
        {logos[info.value.platform] ? (
          <>
            <img
              src={logos[info.value.platform]}
              width={size.value}
              height={size.value}
              alt={info.value.label}
              className={cn(
                (info.value.platform === "github" ||
                  info.value.platform === "codeberg") &&
                  "round-mark",
              )}
            />
          </>
        ) : (
          <>
            {info.value.platform === "local" ? (
              <>
                <span role={"img"} aria-label={info.value.label}>
                  <FolderGit2 size={size.value} />
                </span>
              </>
            ) : (
              <>
                <span role={"img"} aria-label={info.value.label}>
                  <GitBranch size={size.value} />
                </span>
              </>
            )}
          </>
        )}
      </span>
    </>
  );
}
export default observer(function PlatformIcon(props: Props) {
  return useController(createView, props);
});
