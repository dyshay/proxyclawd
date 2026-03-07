export interface InterceptedRequest {
  id: number;
  timestamp: string;
  method: string;
  path: string;
  model: string;
  system_prompt: string | null;
  prompt_text: string;
  response_text: string;
  status: RequestStatus;
  conversation_id: string;
  message_count: number;
  is_tool_loop: boolean;
  is_user_initiated: boolean;
}

export type RequestStatus =
  | "Pending"
  | "Streaming"
  | "Complete"
  | { Error: string };

export type ProxyEvent =
  | {
      NewRequest: {
        id: number;
        timestamp: string;
        method: string;
        path: string;
        model: string;
        system_prompt: string | null;
        prompt_text: string;
        conversation_id: string;
        message_count: number;
        is_tool_loop: boolean;
        is_user_initiated: boolean;
      };
    }
  | { ResponseDelta: { id: number; text: string } }
  | { ResponseComplete: { id: number } }
  | { ResponseError: { id: number; error: string } };

export interface WsSnapshot {
  type: "snapshot";
  requests: InterceptedRequest[];
}

export interface WsEvent {
  type: "event";
  event: ProxyEvent;
}

export type WsMessage = WsSnapshot | WsEvent;
