import { useCallback, useEffect, useState } from "react";

import type { Server } from "../gen/bindings";
import { ask, commands } from "./live";
import { InstallationPicker } from "./InstallationPicker";
import { useInstallations } from "./useInstallations";
import { useShell } from "./useShell";
import { useCopied } from "./useCopied";
import { t } from "../i18n";
import { tRich } from "../i18n/rich";
import { Icon } from './Icon'

/*
 * The MCP servers the agent CLI has, read-only.
 *
 * devpit does not manage MCP: the CLI already owns that catalogue, and two
 * sources of truth diverge on the first `claude mcp add`. No connection dot
 * either — this reads config files, and claiming a server is connected
 * without having connected to it is the kind of thing this panel exists to
 * catch.
 *
 * Which makes naming the installation the whole job. The configuration is not
 * always in `~/.claude`, and a list from the wrong one raises nothing, looks
 * right, and is about a CLI this window will not run.
 */

export function McpPane(): React.JSX.Element {
  const { project, close } = useShell();
  const installations = useInstallations();
  const [servers, setServers] = useState<readonly Server[]>([]);
  const [sources, setSources] = useState<readonly string[]>([]);
  const [directory, setDirectory] = useState("");
  const [manage, setManage] = useState("claude mcp");
  const [problem, setProblem] = useState<string | null>(null);
  const [copied, copyText] = useCopied();

  /* Re-read on every look: `claude mcp add` happens in a terminal beside this
     window, and a catalogue fetched once at startup is wrong by then. */
  const load = useCallback(() => {
    void ask(() =>
      commands.mcpList(project?.id ?? null, installations.chosen),
    ).then((answer) => {
      setServers(answer.data?.servers ?? []);
      setSources(answer.data?.sources ?? []);
      setDirectory(answer.data?.directory ?? "");
      setManage(answer.data?.manageWith ?? "claude mcp");
      setProblem(answer.error ?? answer.data?.problem ?? null);
    });
  }, [project, installations.chosen]);

  useEffect(load, [load]);

  const copy = (): void => copyText(manage);

  const byScope = (which: string): readonly Server[] =>
    servers.filter((server) => server.scope === which);

  return (
    <>
      <div className="pane__bar">
        <span className="pane__ico">
          <Icon size="s">
            <rect x="3" y="8" width="18" height="12" rx="2" />
            <path d="M7 8V5a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v3M8 14h8" />
          </Icon>
        </span>
        <span className="pane__t">
          <b>{t('workspace.mcp.title')}</b>
          {servers.length > 0 ? ` · ${servers.length}` : ""}
        </span>
        <span className="drag"></span>
        <button
          className="sq26"
          onClick={() => {
            installations.reload();
            load();
          }}
          aria-label={t('workspace.mcp.refresh')}
          title={t('workspace.mcp.refresh')}
        >
          <Icon size="s">
            <path d="M21 12a9 9 0 0 1-15.5 6.2M3 12a9 9 0 0 1 15.5-6.2M3 20v-5h5M21 4v5h-5" />
          </Icon>
        </button>
        <button
          className="sq26"
          onClick={() => close("mcps")}
          aria-label={t('workspace.mcp.close')}
          title={t('workspace.mcp.close')}
        >
          <Icon size="s">
            <path d="M18 6 6 18M6 6l12 12" />
          </Icon>
        </button>
      </div>

      <div className="scroll">
        <div className="list">
          <div className="list__in">
            <p className="list__note">
              {tRich("workspace.mcp.note", { command: <code>{manage} add</code> })}{/* // ui-text: ok a command line, not words */}
            </p>

            <InstallationPicker installations={installations} />

            {problem && (
              <div className="exempty">
                <span className="exempty__t">{problem}</span>
                <span className="exempty__d">
                  {t('workspace.mcp.fine')}
                </span>
              </div>
            )}

            {/* Grouped, because the scope is the answer to "why is this one
              here": a project server travels with the repository and a user
              one follows the person to every project they open. */}
            {(["project", "user"] as const).map((scope) =>
              byScope(scope).length === 0 ? null : (
                <section key={scope}>
                  <div className="list__h">
                    {t(scope === "project" ? "workspace.mcp.scope.project" : "workspace.mcp.scope.user")}
                    <span>{byScope(scope).length}</span>
                  </div>
                  {byScope(scope).map((server) => (
                    <div
                      className="cap cap--read"
                      key={`${server.scope}/${server.name}`}
                    >
                      <span className="cap__body">
                        <span className="cap__top">
                          <span className="cap__name">{server.name}</span>
                          <span className="cap__src">{server.scope}</span>
                        </span>
                        <span className="cap__what">{server.reachedBy}</span>
                      </span>
                    </div>
                  ))}
                </section>
              ),
            )}

            <div className="list__h">{t('workspace.mcp.readFrom')}</div>
            <p className="list__path" title={directory}>
              {directory}
            </p>
            {sources.map((source) => (
              <p className="list__path" key={source}>
                {source}
              </p>
            ))}

            <button className="browse" onClick={copy}>
              {t(copied ? "workspace.mcp.copied" : "workspace.mcp.manage", { manage })}
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
