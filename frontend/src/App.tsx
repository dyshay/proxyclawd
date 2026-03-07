import { useCallback, useEffect, useState } from "react";
import RequestList from "./components/RequestList";
import PromptPanel from "./components/PromptPanel";
import ResponsePanel from "./components/ResponsePanel";
import ComposePanel from "./components/ComposePanel";
import { useProxyWebSocket } from "./hooks/useWebSocket";

interface ComposeMode {
  continueConversation: boolean;
}

function App() {
  const { requests, connected } = useProxyWebSocket();
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [autoFollow, setAutoFollow] = useState(true);
  const [collapsedConvs, setCollapsedConvs] = useState<Set<string>>(
    () => new Set()
  );
  const [composeMode, setComposeMode] = useState<ComposeMode | null>(null);

  useEffect(() => {
    if (autoFollow && requests.length > 0) {
      setSelectedId(requests[requests.length - 1].id);
    }
  }, [requests.length, autoFollow]);

  const selectedRequest = requests.find((r) => r.id === selectedId) ?? null;

  const handleSelect = (id: number) => {
    setSelectedId(id);
    setAutoFollow(id === requests[requests.length - 1]?.id);
  };

  const toggleConversation = useCallback((key: string) => {
    setCollapsedConvs((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  // Count active streams
  const streamingCount = requests.filter((r) => r.status === "Streaming").length;
  const completeCount = requests.filter((r) => r.status === "Complete").length;

  // Escape key to close compose
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape" && composeMode) {
        setComposeMode(null);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [composeMode]);

  return (
    <div className="h-screen flex flex-col bg-surface-0">
      {/* Header */}
      <header className="flex items-center justify-between px-5 h-11 border-b border-border-dim bg-surface-1 shrink-0 z-10">
        <div className="flex items-center gap-4">
          <div className="flex items-center gap-2">
            <div className="w-1.5 h-5 bg-accent" />
            <h1 className="font-mono font-bold text-sm text-text-primary tracking-tight uppercase">
              ProxyClawd
            </h1>
          </div>
          <span className="text-[10px] font-mono text-text-tertiary uppercase tracking-[0.15em]">
            MITM Interceptor
          </span>
        </div>

        <div className="flex items-center gap-5">
          {/* Compose buttons */}
          <div className="flex items-center gap-2">
            <button
              onClick={() => setComposeMode({ continueConversation: false })}
              className="px-2.5 py-1 text-[10px] font-mono font-bold uppercase tracking-wider
                bg-surface-3 text-text-secondary hover:bg-accent hover:text-surface-0
                transition-colors cursor-pointer border border-border-dim hover:border-accent"
            >
              New
            </button>
            <button
              onClick={() => setComposeMode({ continueConversation: true })}
              className="px-2.5 py-1 text-[10px] font-mono font-bold uppercase tracking-wider
                bg-surface-3 text-text-secondary hover:bg-accent hover:text-surface-0
                transition-colors cursor-pointer border border-border-dim hover:border-accent"
            >
              Reply
            </button>
          </div>

          {/* Divider */}
          <div className="w-px h-4 bg-border-dim" />

          {/* Stats bar */}
          <div className="flex items-center gap-4 text-[10px] font-mono uppercase tracking-wider">
            <span className="text-text-tertiary">
              REQ <span className="text-text-secondary">{requests.length}</span>
            </span>
            {streamingCount > 0 && (
              <span className="text-status-streaming animate-signal">
                LIVE <span className="font-bold">{streamingCount}</span>
              </span>
            )}
            <span className="text-text-tertiary">
              OK <span className="text-status-complete">{completeCount}</span>
            </span>
          </div>

          {/* Divider */}
          <div className="w-px h-4 bg-border-dim" />

          {/* Auto-follow */}
          <label className="flex items-center gap-2 cursor-pointer group">
            <div
              className={`w-6 h-3 flex items-center transition-colors ${
                autoFollow ? "bg-accent" : "bg-surface-4"
              }`}
            >
              <div
                className={`w-2.5 h-2.5 bg-surface-0 transition-transform ${
                  autoFollow ? "translate-x-3" : "translate-x-0.5"
                }`}
              />
            </div>
            <span className="text-[10px] font-mono text-text-tertiary uppercase tracking-wider group-hover:text-text-secondary transition-colors">
              Follow
            </span>
            <input
              type="checkbox"
              checked={autoFollow}
              onChange={(e) => setAutoFollow(e.target.checked)}
              className="sr-only"
            />
          </label>

          {/* Connection indicator */}
          <div className="flex items-center gap-2">
            <div
              className={`w-1.5 h-1.5 ${
                connected
                  ? "bg-status-complete"
                  : "bg-status-error animate-signal"
              }`}
            />
            <span
              className={`text-[10px] font-mono uppercase tracking-wider ${
                connected ? "text-status-complete" : "text-status-error"
              }`}
            >
              {connected ? "LINK" : "DOWN"}
            </span>
          </div>
        </div>
      </header>

      {/* Main layout */}
      <div className="flex flex-1 min-h-0">
        {/* Sidebar */}
        <div className="w-[340px] shrink-0 flex flex-col min-h-0 border-r border-border-dim">
          <RequestList
            requests={requests}
            selectedId={selectedId}
            onSelect={handleSelect}
            collapsedConvs={collapsedConvs}
            onToggleConversation={toggleConversation}
          />
        </div>

        {/* Content area */}
        <div className="flex-1 flex flex-col min-h-0">
          {composeMode ? (
            <ComposePanel
              continueConversation={composeMode.continueConversation}
              onClose={() => setComposeMode(null)}
              onSent={() => {
                setComposeMode(null);
                setAutoFollow(true);
              }}
            />
          ) : (
            <>
              <div className="h-1/2 border-b border-border-dim min-h-0">
                <PromptPanel request={selectedRequest} />
              </div>
              <div className="h-1/2 min-h-0">
                <ResponsePanel request={selectedRequest} />
              </div>
            </>
          )}
        </div>
      </div>

      {/* Bottom status bar */}
      <footer className="flex items-center justify-between px-5 h-6 border-t border-border-dim bg-surface-1 shrink-0">
        <span className="text-[9px] font-mono text-text-ghost uppercase tracking-widest">
          {selectedRequest
            ? `#${selectedRequest.id} / ${selectedRequest.model} / ${selectedRequest.message_count} messages${selectedRequest.is_user_initiated ? " / USER" : ""}`
            : "No selection"}
        </span>
        <span className="text-[9px] font-mono text-text-ghost uppercase tracking-widest">
          127.0.0.1:8080
        </span>
      </footer>
    </div>
  );
}

export default App;
