import { useEffect, useRef } from "react";
import type { ChatMessage } from "../types/room";

interface ChatFeedProps {
  messages: ChatMessage[];
}

export default function ChatFeed({ messages }: ChatFeedProps) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth", block: "end" });
  }, [messages]);

  return (
    <div className="scrollbar-arcade flex-1 space-y-3 overflow-y-auto px-6 py-4">
      {messages.map((message) => (
        <div key={message.id} className="flex items-start gap-3">
          <span className="font-arcade shrink-0 pt-0.5 text-[9px] text-zinc-500">{message.timestamp}</span>
          <p className="text-sm text-zinc-200">
            <span
              className={
                message.author === "RobotX"
                  ? "font-semibold text-fuchsia-400"
                  : "font-semibold text-emerald-400"
              }
            >
              {message.author}
            </span>
            <span className="text-zinc-500">: </span>
            {message.text}
          </p>
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
}
