import { useEffect } from 'react';
import { useChatStore } from '../../stores/chat';
import { useUIStore } from '../../stores/ui';
import { ConversationsList } from './ConversationsList';
import { ChatView } from './ChatView';

interface ChatViewerProps {
  initialTagId?: string | null;
  initialConversationId?: string | null;
}

export function ChatViewer({ initialTagId, initialConversationId }: ChatViewerProps) {
  const { view, showList, openConversation, reset } = useChatStore();
  const { closeDrawer } = useUIStore();

  // Initialize the chat view based on props
  useEffect(() => {
    if (initialConversationId) {
      // Open specific conversation
      openConversation(initialConversationId);
    } else {
      // Show list, optionally filtered by tag
      showList(initialTagId ?? undefined);
    }

    // Cleanup on unmount
    return () => {
      reset();
    };
  }, [initialTagId, initialConversationId, showList, openConversation, reset]);

  return (
    <div className="h-full flex flex-col bg-[#252525]">
      {/* Header */}
      <div className="flex-shrink-0 flex items-center justify-between px-4 py-3 border-b border-[#3d3d3d]">
        <h2 className="text-lg font-semibold text-[#dcddde]">
          {view === 'list' ? 'Conversations' : 'Chat'}
        </h2>
        <button
          onClick={closeDrawer}
          className="p-1 text-[#888888] hover:text-[#dcddde] transition-colors"
          aria-label="Close"
        >
          <svg
            className="w-5 h-5"
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-hidden">
        {view === 'list' ? <ConversationsList /> : <ChatView />}
      </div>
    </div>
  );
}
