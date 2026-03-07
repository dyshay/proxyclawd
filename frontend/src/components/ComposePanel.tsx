import { useEffect, useRef, useState } from "react";

interface Props {
  continueConversation: boolean;
  onClose: () => void;
  onSent: () => void;
}

export default function ComposePanel({
  continueConversation,
  onClose,
  onSent,
}: Props) {
  const [message, setMessage] = useState("");
  const [sending, setSending] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Focus textarea on mount
  useEffect(() => {
    textareaRef.current?.focus();
  }, []);

  const handleSend = async () => {
    if (!message.trim() || sending) return;
    setSending(true);
    setStatus(null);

    try {
      const res = await fetch("/api/send", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message,
          continue_conversation: continueConversation,
        }),
      });
      if (res.ok) {
        setStatus("Sent to Claude Code — response will appear via proxy");
        setTimeout(() => onSent(), 1000);
      } else {
        const data = await res.json();
        setStatus(data.error || "Send failed");
        setSending(false);
      }
    } catch {
      setStatus("Network error");
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key === "Enter") {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="px-4 py-2.5 border-b border-border-dim bg-surface-1 flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          <div className="w-1.5 h-5 bg-accent" />
          <span className="text-[10px] font-mono font-bold text-accent uppercase tracking-[0.15em]">
            {continueConversation ? "Reply (--continue)" : "New Message"}
          </span>
        </div>
        <button
          onClick={onClose}
          className="text-[10px] font-mono text-text-ghost hover:text-text-secondary transition-colors uppercase tracking-wider cursor-pointer"
        >
          Cancel (Esc)
        </button>
      </div>

      {/* Message input */}
      <div className="flex-1 p-4 flex flex-col min-h-0">
        <textarea
          ref={textareaRef}
          value={message}
          onChange={(e) => setMessage(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Type your message..."
          className="flex-1 w-full bg-surface-2 border border-border-dim text-[12px] text-text-primary font-mono p-4 outline-none focus:border-accent resize-none placeholder:text-text-ghost leading-[1.7]"
        />
      </div>

      {/* Footer */}
      <div className="px-4 py-2.5 border-t border-border-dim bg-surface-1 flex items-center justify-between shrink-0">
        <div className="flex items-center gap-3">
          {status && (
            <span className="text-[10px] font-mono text-text-tertiary uppercase tracking-wider">
              {status}
            </span>
          )}
          {!status && (
            <span className="text-[10px] font-mono text-text-ghost uppercase tracking-wider">
              Ctrl+Enter to send
            </span>
          )}
        </div>
        <button
          onClick={handleSend}
          disabled={!message.trim() || sending}
          className={`px-4 py-1.5 text-[10px] font-mono font-bold uppercase tracking-wider transition-colors cursor-pointer
            ${
              message.trim() && !sending
                ? "bg-accent text-surface-0 hover:bg-accent-dim"
                : "bg-surface-3 text-text-ghost cursor-not-allowed"
            }`}
        >
          {sending ? "Sending..." : "Send"}
        </button>
      </div>
    </div>
  );
}
