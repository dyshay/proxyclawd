import { useMemo, useEffect, useRef } from "react";
import type { InterceptedRequest } from "../types";

function StatusBar({ status }: { status: InterceptedRequest["status"] }) {
  if (status === "Pending")
    return <div className="w-0.5 h-3 bg-status-pending" />;
  if (status === "Streaming")
    return <div className="w-0.5 h-3 bg-status-streaming animate-signal" />;
  if (status === "Complete")
    return <div className="w-0.5 h-3 bg-status-complete" />;
  return <div className="w-0.5 h-3 bg-status-error" />;
}

function formatTime(ts: string) {
  const d = new Date(ts);
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

function truncModel(model: string) {
  if (model.length > 28) return model.slice(0, 28) + "\u2026";
  return model;
}

type DisplayRow =
  | {
      kind: "conv-header";
      conversationId: string;
      count: number;
      lastStatus: InterceptedRequest["status"];
      firstTimestamp: string;
    }
  | { kind: "request"; request: InterceptedRequest; indent: number }
  | { kind: "tool-loop-header"; key: string; count: number; indent: number };

function buildThreadRows(
  requests: InterceptedRequest[],
  collapsedConvs: Set<string>
): DisplayRow[] {
  if (requests.length === 0) return [];

  const convOrder: string[] = [];
  const convGroups = new Map<string, InterceptedRequest[]>();

  for (const req of requests) {
    const cid = req.conversation_id;
    if (!convGroups.has(cid)) {
      convOrder.push(cid);
      convGroups.set(cid, []);
    }
    convGroups.get(cid)!.push(req);
  }

  const rows: DisplayRow[] = [];

  for (const cid of convOrder) {
    const group = convGroups.get(cid)!;
    const sorted = [...group].sort((a, b) => a.message_count - b.message_count);
    const isCollapsed = collapsedConvs.has(cid);

    if (sorted.length === 1) {
      rows.push({ kind: "request", request: sorted[0], indent: 0 });
      continue;
    }

    const lastStatus = sorted[sorted.length - 1].status;
    rows.push({
      kind: "conv-header",
      conversationId: cid,
      count: sorted.length,
      lastStatus,
      firstTimestamp: sorted[0].timestamp,
    });

    if (isCollapsed) continue;

    let i = 0;
    while (i < sorted.length) {
      if (sorted[i].is_tool_loop) {
        const runStart = i;
        while (i < sorted.length && sorted[i].is_tool_loop) i++;
        const runLen = i - runStart;
        const tlKey = `${cid}-tl-${runStart}`;
        const tlCollapsed = collapsedConvs.has(tlKey);

        rows.push({
          kind: "tool-loop-header",
          key: tlKey,
          count: runLen,
          indent: 1,
        });

        if (!tlCollapsed) {
          for (let j = runStart; j < runStart + runLen; j++) {
            rows.push({ kind: "request", request: sorted[j], indent: 2 });
          }
        }
      } else {
        rows.push({ kind: "request", request: sorted[i], indent: 1 });
        i++;
      }
    }
  }

  return rows;
}

interface Props {
  requests: InterceptedRequest[];
  selectedId: number | null;
  onSelect: (id: number) => void;
  collapsedConvs: Set<string>;
  onToggleConversation: (key: string) => void;
}

export default function RequestList({
  requests,
  selectedId,
  onSelect,
  collapsedConvs,
  onToggleConversation,
}: Props) {
  const rows = useMemo(
    () => buildThreadRows(requests, collapsedConvs),
    [requests, collapsedConvs]
  );

  const scrollRef = useRef<HTMLDivElement>(null);
  const prevLength = useRef(requests.length);

  // Auto-scroll when new requests arrive
  useEffect(() => {
    if (requests.length > prevLength.current && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
    prevLength.current = requests.length;
  }, [requests.length]);

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-4 py-2.5 border-b border-border-dim bg-surface-1 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-[10px] font-mono font-bold text-text-secondary uppercase tracking-[0.15em]">
            Intercepts
          </span>
        </div>
        <span className="text-[10px] font-mono text-text-ghost tabular-nums">
          {requests.length}
        </span>
      </div>

      {/* List */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto">
        {requests.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full gap-3 px-6">
            <div className="font-mono text-text-ghost text-xs text-center leading-relaxed">
              <div className="text-text-tertiary mb-2">[IDLE]</div>
              <div>Waiting for requests</div>
              <div className="mt-2 text-[10px]">
                HTTPS_PROXY=http://127.0.0.1:8080
              </div>
            </div>
          </div>
        )}

        {rows.map((row, idx) => {
          if (row.kind === "conv-header") {
            const isCollapsed = collapsedConvs.has(row.conversationId);
            const convShort = row.conversationId.slice(0, 8);
            return (
              <button
                key={`conv-${row.conversationId}`}
                onClick={() => onToggleConversation(row.conversationId)}
                className="w-full text-left px-4 py-2 border-b border-border-dim
                  hover:bg-accent-ghost transition-all cursor-pointer
                  bg-surface-1 group animate-fade-in"
              >
                <div className="flex items-center gap-2.5">
                  <span
                    className="text-[10px] text-text-tertiary transition-transform inline-block group-hover:text-accent"
                    style={{
                      transform: isCollapsed
                        ? "rotate(0deg)"
                        : "rotate(90deg)",
                    }}
                  >
                    &#9654;
                  </span>
                  <StatusBar status={row.lastStatus} />
                  <span className="text-[10px] font-mono text-accent font-semibold uppercase tracking-wider">
                    {convShort}
                  </span>
                  <span className="text-[10px] font-mono text-text-ghost">
                    {row.count} req{row.count > 1 ? "s" : ""}
                  </span>
                  <span className="ml-auto text-[10px] font-mono text-text-ghost tabular-nums">
                    {formatTime(row.firstTimestamp)}
                  </span>
                </div>
              </button>
            );
          }

          if (row.kind === "tool-loop-header") {
            const isCollapsed = collapsedConvs.has(row.key);
            return (
              <button
                key={`tl-${row.key}-${idx}`}
                onClick={() => onToggleConversation(row.key)}
                className="w-full text-left py-1.5 border-b border-border-dim/50
                  hover:bg-accent-ghost transition-all cursor-pointer group animate-fade-in"
                style={{ paddingLeft: `${row.indent * 20 + 16}px` }}
              >
                <div className="flex items-center gap-2">
                  <div className="w-3 border-t border-border" />
                  <span
                    className="text-[9px] text-text-ghost transition-transform inline-block group-hover:text-text-tertiary"
                    style={{
                      transform: isCollapsed
                        ? "rotate(0deg)"
                        : "rotate(90deg)",
                    }}
                  >
                    &#9654;
                  </span>
                  <span className="text-[10px] font-mono text-text-ghost uppercase tracking-wider">
                    tool loop
                  </span>
                  <span className="text-[10px] font-mono text-text-ghost">&times;{row.count}</span>
                </div>
              </button>
            );
          }

          // kind === "request"
          const req = row.request;
          const isSelected = selectedId === req.id;
          const isStreaming = req.status === "Streaming";

          return (
            <button
              key={req.id}
              onClick={() => onSelect(req.id)}
              className={`w-full text-left py-2 border-b transition-all cursor-pointer
                animate-fade-in group relative
                ${isSelected
                  ? "bg-accent-ghost border-border"
                  : "border-border-dim/50 hover:bg-surface-2"
                }
                ${isStreaming ? "bg-surface-2" : ""}
              `}
              style={{
                paddingLeft: `${row.indent * 20 + 16}px`,
                paddingRight: "16px",
              }}
            >
              {/* Active selection indicator */}
              {isSelected && (
                <div className="absolute left-0 top-0 bottom-0 w-0.5 bg-accent" />
              )}

              {/* Indent connector */}
              {row.indent > 0 && (
                <div
                  className="absolute top-0 bottom-0 w-px bg-border-dim"
                  style={{ left: `${(row.indent - 1) * 20 + 22}px` }}
                />
              )}

              <div className="flex items-center gap-2">
                <StatusBar status={req.status} />
                <span className="text-[10px] font-mono text-text-ghost tabular-nums">
                  {formatTime(req.timestamp)}
                </span>
                <span className="text-[10px] font-mono text-text-tertiary tabular-nums">
                  {req.message_count}m
                </span>
                {isStreaming && (
                  <span className="text-[10px] font-mono text-status-streaming animate-signal uppercase tracking-wider">
                    live
                  </span>
                )}
                {typeof req.status === "object" && "Error" in req.status && (
                  <span className="text-[10px] font-mono text-status-error uppercase tracking-wider truncate max-w-[80px]">
                    {req.status.Error}
                  </span>
                )}
              </div>
              <div className="mt-0.5 flex items-center gap-1.5" style={{ marginLeft: "6px" }}>
                {req.is_user_initiated && (
                  <span className="text-[9px] font-mono text-cyan-400 border border-cyan-400/30 px-1 uppercase font-bold">
                    user
                  </span>
                )}
                <span
                  className={`text-[11px] font-mono truncate ${
                    isSelected ? "text-text-primary" : "text-text-secondary"
                  }`}
                >
                  {truncModel(req.model)}
                </span>
                {req.is_tool_loop && (
                  <span className="text-[9px] font-mono text-text-ghost border border-border-dim px-1 uppercase">
                    tool
                  </span>
                )}
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
