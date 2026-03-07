import { useEffect, useState } from "react";
import RequestList from "./components/RequestList";
import PromptPanel from "./components/PromptPanel";
import ResponsePanel from "./components/ResponsePanel";
import { useProxyWebSocket } from "./hooks/useWebSocket";

function App() {
  const { requests, connected } = useProxyWebSocket();
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [autoFollow, setAutoFollow] = useState(true);

  // Auto-select latest request when new ones arrive
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

  return (
    <div className="h-screen flex flex-col">
      {/* Header */}
      <header className="flex items-center justify-between px-4 py-2 border-b border-gray-800 bg-gray-950 shrink-0">
        <div className="flex items-center gap-3">
          <h1 className="font-bold text-lg text-white tracking-tight">
            ProxyClawd
          </h1>
          <span className="text-xs text-gray-500">MITM Proxy Viewer</span>
        </div>
        <div className="flex items-center gap-3">
          <label className="flex items-center gap-1.5 text-xs text-gray-500 cursor-pointer">
            <input
              type="checkbox"
              checked={autoFollow}
              onChange={(e) => setAutoFollow(e.target.checked)}
              className="accent-purple-500"
            />
            Auto-follow
          </label>
          <div
            className={`w-2 h-2 rounded-full ${connected ? "bg-green-400" : "bg-red-400 animate-pulse"}`}
            title={connected ? "Connected" : "Disconnected"}
          />
        </div>
      </header>

      {/* Main layout */}
      <div className="flex flex-1 min-h-0">
        {/* Sidebar: request list */}
        <div className="w-80 shrink-0 flex flex-col min-h-0">
          <RequestList
            requests={requests}
            selectedId={selectedId}
            onSelect={handleSelect}
          />
        </div>

        {/* Content area */}
        <div className="flex-1 flex flex-col min-h-0">
          {/* Top: Prompt */}
          <div className="h-1/2 border-b border-gray-800 min-h-0">
            <PromptPanel request={selectedRequest} />
          </div>
          {/* Bottom: Response */}
          <div className="h-1/2 min-h-0">
            <ResponsePanel request={selectedRequest} />
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
