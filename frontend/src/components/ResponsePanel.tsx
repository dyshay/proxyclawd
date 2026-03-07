import { useEffect, useRef } from "react";
import type { InterceptedRequest } from "../types";

interface Props {
  request: InterceptedRequest | null;
}

export default function ResponsePanel({ request }: Props) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (request?.status === "Streaming") {
      bottomRef.current?.scrollIntoView({ behavior: "smooth" });
    }
  }, [request?.response_text, request?.status]);

  if (!request) {
    return (
      <div className="flex items-center justify-center h-full">
        <span className="text-[11px] font-mono text-text-ghost uppercase tracking-widest">
          Select a request
        </span>
      </div>
    );
  }

  const isStreaming = request.status === "Streaming";
  const isComplete = request.status === "Complete";
  const isError =
    typeof request.status === "object" && "Error" in request.status;
  const isPending = request.status === "Pending";

  return (
    <div className="flex flex-col h-full">
      {/* Panel header */}
      <div className="px-4 py-2 border-b border-border-dim bg-surface-1 flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          <span className="text-[10px] font-mono font-bold text-text-secondary uppercase tracking-[0.15em]">
            Response
          </span>
          {isStreaming && (
            <span className="text-[10px] font-mono text-status-streaming animate-signal uppercase tracking-wider">
              Streaming
            </span>
          )}
          {isComplete && (
            <span className="text-[10px] font-mono text-status-complete uppercase tracking-wider">
              Complete
            </span>
          )}
          {isError && (
            <span className="text-[10px] font-mono text-status-error uppercase tracking-wider">
              Error
            </span>
          )}
          {isPending && (
            <span className="text-[10px] font-mono text-text-ghost uppercase tracking-wider">
              Pending
            </span>
          )}
        </div>
        {request.response_text && (
          <span className="text-[10px] font-mono text-text-ghost tabular-nums">
            {request.response_text.length.toLocaleString()} chars
          </span>
        )}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto p-4">
        {isPending ? (
          <div className="flex items-center justify-center h-full">
            <div className="flex flex-col items-center gap-2">
              <div className="flex gap-1">
                <div className="w-1 h-4 bg-text-ghost animate-signal" style={{ animationDelay: "0ms" }} />
                <div className="w-1 h-4 bg-text-ghost animate-signal" style={{ animationDelay: "200ms" }} />
                <div className="w-1 h-4 bg-text-ghost animate-signal" style={{ animationDelay: "400ms" }} />
              </div>
              <span className="text-[10px] font-mono text-text-ghost uppercase tracking-widest">
                Waiting
              </span>
            </div>
          </div>
        ) : (
          <div className="animate-fade-in">
            {isStreaming && (
              <div className="w-full h-px bg-gradient-to-r from-status-streaming/50 via-status-streaming to-status-streaming/50 mb-4 animate-signal" />
            )}
            <pre className="whitespace-pre-wrap text-[12px] text-text-primary font-mono leading-[1.7]">
              {request.response_text}
            </pre>
            {isError && typeof request.status === "object" && "Error" in request.status && (
              <div className="mt-4 p-3 border border-status-error/30 bg-status-error/5">
                <span className="text-[10px] font-mono text-status-error uppercase tracking-wider">
                  {request.status.Error}
                </span>
              </div>
            )}
          </div>
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
