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
      <div className="flex items-center justify-center h-full text-gray-600 text-sm">
        Select a request to view the response
      </div>
    );
  }

  const statusLabel = () => {
    if (request.status === "Pending")
      return <span className="text-gray-500">waiting...</span>;
    if (request.status === "Streaming")
      return (
        <span className="text-yellow-400 animate-pulse">
          &bull; streaming
        </span>
      );
    if (request.status === "Complete")
      return <span className="text-green-400">complete</span>;
    if (typeof request.status === "object" && "Error" in request.status)
      return <span className="text-red-400">error</span>;
    return null;
  };

  return (
    <div className="flex flex-col h-full">
      <div className="px-4 py-2 border-b border-gray-800 font-semibold text-sm text-gray-400 uppercase tracking-wide flex items-center gap-2">
        Response
        {statusLabel()}
      </div>
      <div className="flex-1 overflow-y-auto p-4">
        {request.status === "Pending" ? (
          <div className="text-gray-600 text-sm">Waiting for response...</div>
        ) : (
          <pre className="whitespace-pre-wrap text-sm text-gray-200 font-mono leading-relaxed">
            {request.response_text}
          </pre>
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
