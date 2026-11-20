import github from "../../assets/platforms/github.svg";

export function Github({ size = 16 }: { size?: number }) {
  return (
    <img
      className="github-icon"
      src={github}
      width={size}
      height={size}
      alt=""
      aria-hidden="true"
    />
  );
}
