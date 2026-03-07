import { useState } from "react";
import type { InterceptedRequest } from "../types";

interface Props {
  request: InterceptedRequest | null;
}

export default function PromptPanel({ request }: Props) {
  const [systemCollapsed, setSystemCollapsed] = useState(false);

  if (!request) {
    return (
      <div className="flex items-center justify-center h-full">
        <span className="text-[11px] font-mono text-text-ghost uppercase tracking-widest">
          Select a request
        </span>
      </div>
    );
  }

  const charCount = (request.prompt_text?.length ?? 0) + (request.system_prompt?.length ?? 0);

  return (
    <div className="flex flex-col h-full">
      {/* Panel header */}
      <div className="px-4 py-2 border-b border-border-dim bg-surface-1 flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          <span className="text-[10px] font-mono font-bold text-text-secondary uppercase tracking-[0.15em]">
            Prompt
          </span>
          <div className="flex items-center gap-2">
            <span className="text-[10px] font-mono text-text-ghost tabular-nums">
              {charCount.toLocaleString()} chars
            </span>
            <span className="text-[10px] font-mono text-text-ghost tabular-nums">
              {request.message_count} msg
            </span>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4 space-y-4">
        {/* User message */}
        <div className="animate-fade-in">
          <div className="flex items-center gap-2 mb-2">
            <div className="w-1 h-3 bg-accent" />
            <span className="text-[10px] font-mono font-bold text-accent uppercase tracking-[0.15em]">
              User
            </span>
          </div>
          <pre className="whitespace-pre-wrap text-[12px] text-text-primary font-mono leading-[1.7] bg-surface-2 p-4 border border-border-dim">
            {request.prompt_text || "(empty)"}
          </pre>
        </div>

        {/* System prompt */}
        {request.system_prompt && (
          <div className="animate-fade-in" style={{ animationDelay: "50ms" }}>
            <button
              onClick={() => setSystemCollapsed(!systemCollapsed)}
              className="flex items-center gap-2 mb-2 cursor-pointer group"
            >
              <div className="w-1 h-3 bg-text-tertiary" />
              <span className="text-[10px] font-mono font-bold text-text-tertiary uppercase tracking-[0.15em] group-hover:text-text-secondary transition-colors">
                System
              </span>
              <span className="text-[10px] font-mono text-text-ghost tabular-nums">
                {request.system_prompt.length.toLocaleString()} chars
              </span>
              <span
                className="text-[9px] text-text-ghost transition-transform inline-block group-hover:text-text-tertiary"
                style={{
                  transform: systemCollapsed
                    ? "rotate(0deg)"
                    : "rotate(90deg)",
                }}
              >
                &#9654;
              </span>
            </button>
            {!systemCollapsed && (
              <pre className="whitespace-pre-wrap text-[11px] text-text-tertiary font-mono leading-[1.7] bg-surface-2 p-4 border border-border-dim max-h-96 overflow-y-auto">
                {request.system_prompt}
              </pre>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
