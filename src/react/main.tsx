import { createRoot } from "react-dom/client";
import App from "./App";
import "./shadcn.css";
import "../style.css";
import "../theme.css";
import "./workspace.css";
import { captureLog, flushLogs, startDiagnostics } from "../lib/diagnostics";

const stopDiagnostics = startDiagnostics();
const root = createRoot(document.getElementById("app")!);
root.render(<App />);

if (import.meta.hot)
  import.meta.hot.dispose(() => {
    root.unmount();
    stopDiagnostics();
  });
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    captureLog("info", "app", "Frontend mounted");
    void flushLogs();
  }),
);
