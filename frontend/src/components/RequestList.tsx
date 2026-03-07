import type { InterceptedRequest } from "../types";

function statusIcon(status: InterceptedRequest["status"]) {
  if (status === "Pending") return <span className="text-gray-500">.</span>;
  if (status === "Streaming")
    return <span className="text-yellow-400 animate-pulse font-bold">&bull;</span>;
  if (status === "Complete")
    return <span className="text-green-400">&check;</span>;
  return <span className="text-red-400">!</span>;
}

function formatTime(ts: string) {
  const d = new Date(ts);
  return d.toLocaleTimeString("en-GB", { hour12: false });
}

function truncModel(model: string) {
  return model.length > 24 ? model.slice(0, 24) + "..." : model;
}

interface Props {
  requests: InterceptedRequest[];
  selectedId: number | null;
  onSelect: (id: number) => void;
}

export default function RequestList({ requests, selectedId, onSelect }: Props) {
  return (
    <div className="flex flex-col h-full border-r border-gray-800">
      <div className="px-3 py-2 border-b border-gray-800 font-semibold text-sm text-gray-400 uppercase tracking-wide">
        Requests ({requests.length})
      </div>
      <div className="flex-1 overflow-y-auto">
        {requests.length === 0 && (
          <div className="px-3 py-8 text-center text-gray-600 text-sm">
            Waiting for requests...
          </div>
        )}
        {requests.map((req) => (
          <button
            key={req.id}
            onClick={() => onSelect(req.id)}
            className={`w-full text-left px-3 py-2 border-b border-gray-800/50 hover:bg-gray-800/60 transition-colors cursor-pointer ${
              selectedId === req.id ? "bg-gray-800" : ""
            }`}
          >
            <div className="flex items-center gap-2 text-sm">
              <span className="w-4 text-center">{statusIcon(req.status)}</span>
              <span className="text-gray-500 font-mono text-xs">
                {formatTime(req.timestamp)}
              </span>
              <span className="text-cyan-400 font-mono text-xs truncate">
                {req.method} {req.path}
              </span>
            </div>
            <div className="flex items-center gap-2 mt-0.5 ml-6">
              <span className="text-purple-400 text-xs">
                {truncModel(req.model)}
              </span>
              {req.status === "Streaming" && (
                <span className="text-yellow-400 text-xs">streaming...</span>
              )}
              {typeof req.status === "object" && "Error" in req.status && (
                <span className="text-red-400 text-xs truncate">
                  err: {req.status.Error}
                </span>
              )}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
