import { useEffect, useState } from "react";
import { App } from "./App";
import { PackagesApp } from "./PackagesApp";

export type Workspace = "packages" | "studies";

/**
 * Which workspace a link opens: studies when it names a problem, as every
 * study link does, or asks for them (`?studies`); packages otherwise.
 */
export function linkedWorkspace(search = window.location.search): Workspace {
  const query = new URLSearchParams(search);
  return query.has("problem") || query.has("studies") ? "studies" : "packages";
}

/**
 * Studio: packages, points, refs and experiments by default (P12), and the
 * studies of the sample problems while they remain.
 */
export function Studio() {
  const [workspace, setWorkspace] = useState<Workspace>(() => linkedWorkspace());
  useEffect(() => {
    const restore = () => setWorkspace(linkedWorkspace());
    window.addEventListener("popstate", restore);
    return () => window.removeEventListener("popstate", restore);
  }, []);
  const open = (next: Workspace) => {
    if (next === workspace) return;
    window.history.pushState({}, "", `${window.location.pathname}${next === "studies" ? "?studies" : ""}${window.location.hash}`);
    setWorkspace(next);
  };
  const switcher = <nav className="workspace-switch" aria-label="Workspace">
    <button type="button" aria-current={workspace === "packages" ? "page" : undefined} onClick={() => open("packages")}>Packages</button>
    <button type="button" aria-current={workspace === "studies" ? "page" : undefined} onClick={() => open("studies")}>Studies</button>
  </nav>;
  return workspace === "studies" ? <App workspace={switcher} /> : <PackagesApp workspace={switcher} />;
}
