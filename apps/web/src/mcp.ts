import { App } from "@modelcontextprotocol/ext-apps";
import { setBridge } from "./api/bridge";
import { createMcpBridge } from "./api/mcp";
import { setHostTheme } from "./components/ThemeToggle";
import { mountApp } from "./render";

const app = new App(
  { name: "Distill", version: "0.0.1" },
  { availableDisplayModes: ["fullscreen"] },
);
app.onhostcontextchanged = (context) => setHostTheme(context.theme);
app.ontoolresult = () => {
  document.title = app.getHostContext()?.toolInfo?.tool.title ?? "Distill";
};

async function start(): Promise<void> {
  await app.connect();
  setHostTheme(app.getHostContext()?.theme);
  document.title = app.getHostContext()?.toolInfo?.tool.title ?? "Distill";
  setBridge(createMcpBridge(app));
  mountApp();
}

void start().catch((error: unknown) => {
  const root = document.getElementById("root");
  if (root) root.textContent = `Distill could not connect to the host: ${String(error)}`;
});
