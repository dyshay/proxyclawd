import { useState } from "react";
import type { InterceptedRequest } from "../types";

interface Props {
  request: InterceptedRequest | null;
}

export default function PromptPanel({ request }: Props) {
  const [systemCollapsed, setSystemCollapsed] = useState(false);

  if (!request) {
    return (
      <div className="flex items-center justify-center h-full text-gray-600 text-sm">
        Select a request to view the prompt
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-y-auto">
      <div className="px-4 py-2 border-b border-gray-800 font-semibold text-sm text-gray-400 uppercase tracking-wide">
        Prompt
      </div>
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* User message */}
        <div>
          <div className="text-xs font-semibold text-cyan-400 mb-1 uppercase tracking-wide">
            User
          </div>
          <pre className="whitespace-pre-wrap text-sm text-gray-200 font-mono leading-relaxed bg-gray-900 rounded p-3">
            {request.prompt_text || "(empty)"}
          </pre>
        </div>

        {/* System prompt */}
        {request.system_prompt && (
          <div>
            <button
              onClick={() => setSystemCollapsed(!systemCollapsed)}
              className="text-xs font-semibold text-purple-400 mb-1 uppercase tracking-wide cursor-pointer hover:text-purple-300 flex items-center gap-1"
            >
              <span>{systemCollapsed ? "\u25b6" : "\u25bc"}</span>
              System
              <span className="text-gray-600 normal-case font-normal ml-1">
                ({request.system_prompt.length.toLocaleString()} chars)
              </span>
            </button>
            {!systemCollapsed && (
              <pre className="whitespace-pre-wrap text-sm text-gray-400 font-mono leading-relaxed bg-gray-900 rounded p-3 max-h-96 overflow-y-auto">
                {request.system_prompt}
              </pre>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
