import github from "../../assets/platforms/github.svg";

export function Github({ size = 16 }: { size?: number }) {
  return <ThemeIcon src={github} size={size} />;
}

export function ThemeIcon({ src, size }: { src: string; size: number }) {
  return (
    <span
      className="theme-icon"
      style={{ width: size, height: size, maskImage: `url("${src}")` }}
      aria-hidden="true"
    />
  );
}
