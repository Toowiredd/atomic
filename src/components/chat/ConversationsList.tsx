import { useChatStore, ConversationWithTags } from '../../stores/chat';
import { ConversationCard } from './ConversationCard';

export function ConversationsList() {
  const {
    conversations,
    isLoading,
    error,
    listFilterTagId,
    createConversation,
    openConversation,
    deleteConversation,
  } = useChatStore();

  const handleNewChat = async () => {
    try {
      // Create conversation with current filter tag if any
      const tagIds = listFilterTagId ? [listFilterTagId] : [];
      await createConversation(tagIds);
    } catch (e) {
      console.error('Failed to create conversation:', e);
    }
  };

  const handleOpenConversation = (conversation: ConversationWithTags) => {
    openConversation(conversation.id);
  };

  const handleDeleteConversation = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (confirm('Delete this conversation?')) {
      await deleteConversation(id);
    }
  };

  if (isLoading && conversations.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-[#888888]">
        Loading conversations...
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex flex-col items-center justify-center h-full gap-4 p-4">
        <p className="text-red-400">{error}</p>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* New Chat Button */}
      <div className="flex-shrink-0 p-4 border-b border-[#3d3d3d]">
        <button
          onClick={handleNewChat}
          className="w-full flex items-center justify-center gap-2 px-4 py-2.5 bg-[#7c3aed] hover:bg-[#6d28d9] text-white rounded-lg transition-colors"
        >
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          New Conversation
        </button>
      </div>

      {/* Conversations List */}
      <div className="flex-1 overflow-y-auto">
        {conversations.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-4 p-8 text-center">
            <div className="w-16 h-16 rounded-full bg-[#2d2d2d] flex items-center justify-center">
              <svg className="w-8 h-8 text-[#888888]" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
              </svg>
            </div>
            <div>
              <p className="text-[#dcddde] font-medium mb-1">No conversations yet</p>
              <p className="text-[#888888] text-sm">
                Start a new conversation to chat with your knowledge base
              </p>
            </div>
          </div>
        ) : (
          <div className="divide-y divide-[#3d3d3d]">
            {conversations.map((conversation) => (
              <ConversationCard
                key={conversation.id}
                conversation={conversation}
                onClick={() => handleOpenConversation(conversation)}
                onDelete={(e) => handleDeleteConversation(conversation.id, e)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
