import { useCallback, useEffect, useRef, useState } from "react";
import type {
  InterceptedRequest,
  ProxyEvent,
  WsMessage,
} from "../types";

function applyEvent(
  requests: InterceptedRequest[],
  event: ProxyEvent
): InterceptedRequest[] {
  if ("NewRequest" in event) {
    const e = event.NewRequest;
    return [
      ...requests,
      {
        id: e.id,
        timestamp: e.timestamp,
        method: e.method,
        path: e.path,
        model: e.model,
        system_prompt: e.system_prompt,
        prompt_text: e.prompt_text,
        response_text: "",
        status: "Pending",
      },
    ];
  }

  if ("ResponseDelta" in event) {
    const { id, text } = event.ResponseDelta;
    return requests.map((r) =>
      r.id === id
        ? { ...r, response_text: r.response_text + text, status: "Streaming" as const }
        : r
    );
  }

  if ("ResponseComplete" in event) {
    const { id } = event.ResponseComplete;
    return requests.map((r) =>
      r.id === id ? { ...r, status: "Complete" as const } : r
    );
  }

  if ("ResponseError" in event) {
    const { id, error } = event.ResponseError;
    return requests.map((r) =>
      r.id === id ? { ...r, status: { Error: error } } : r
    );
  }

  return requests;
}

export function useProxyWebSocket() {
  const [requests, setRequests] = useState<InterceptedRequest[]>([]);
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const connect = useCallback(() => {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const url = `${protocol}//${window.location.host}/ws`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => setConnected(true);

    ws.onmessage = (evt) => {
      const msg: WsMessage = JSON.parse(evt.data);
      if (msg.type === "snapshot") {
        setRequests(msg.requests);
      } else if (msg.type === "event") {
        setRequests((prev) => applyEvent(prev, msg.event));
      }
    };

    ws.onclose = () => {
      setConnected(false);
      reconnectTimer.current = setTimeout(connect, 2000);
    };

    ws.onerror = () => ws.close();
  }, []);

  useEffect(() => {
    connect();
    return () => {
      clearTimeout(reconnectTimer.current);
      wsRef.current?.close();
    };
  }, [connect]);

  return { requests, connected };
}
