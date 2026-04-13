import { useState, useRef, useEffect, useCallback, useMemo } from 'react';
import { MessageSquare, Search as SearchIcon } from 'lucide-react';
import { useChatStore } from '../../stores/chat';
import { useUIStore } from '../../stores/ui';
import { useChatEvents } from '../../hooks/useChatEvents';
import { useContentSearch } from '../../hooks';
import { ChatHeader } from './ChatHeader';
import { ChatMessage } from './ChatMessage';
import { ChatInput } from './ChatInput';
import { SearchBar } from '../ui/SearchBar';

export function ChatView() {
  const currentConversation = useChatStore(s => s.currentConversation);
  const messages = useChatStore(s => s.messages);
  const isLoading = useChatStore(s => s.isLoading);
  const isStreaming = useChatStore(s => s.isStreaming);
  const streamingContent = useChatStore(s => s.streamingContent);
  const retrievalSteps = useChatStore(s => s.retrievalSteps);
  const error = useChatStore(s => s.error);
  const sendMessage = useChatStore(s => s.sendMessage);
  const goBack = useChatStore(s => s.goBack);

  const openReader = useUIStore(s => s.openReader);

  const [inputValue, setInputValue] = useState('');
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const messagesContainerRef = useRef<HTMLDivElement>(null);
  const isNearBottomRef = useRef(true);

  // Combine all message content for search
  const allContent = useMemo(() => {
    return messages.map(m => m.content).join('\n\n');
  }, [messages]);

  // Content search across all messages
  const {
    isOpen: isSearchOpen,
    query: searchQuery,
    searchedQuery,
    currentIndex,
    totalMatches,
    setQuery: setSearchQuery,
    openSearch,
    closeSearch,
    goToNext,
    goToPrevious,
    highlightText,
  } = useContentSearch(allContent);

  // Keyboard handler for Ctrl+F / Cmd+F
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault();
        openSearch();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [openSearch]);

  // Subscribe to chat events for streaming
  useChatEvents(currentConversation?.id ?? null);

  // Check if user is near the bottom of the scroll container
  const checkIfNearBottom = useCallback(() => {
    const container = messagesContainerRef.current;
    if (!container) return true;

    const threshold = 100; // pixels from bottom to consider "at bottom"
    const { scrollTop, scrollHeight, clientHeight } = container;
    return scrollHeight - scrollTop - clientHeight < threshold;
  }, []);

  // Update near-bottom state on scroll
  const handleScroll = useCallback(() => {
    isNearBottomRef.current = checkIfNearBottom();
  }, [checkIfNearBottom]);

  // Auto-scroll to bottom only if user is already near bottom
  useEffect(() => {
    if (isNearBottomRef.current) {
      messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    }
  }, [messages, streamingContent]);

  // Always scroll to bottom when a new message is sent (user action)
  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
    isNearBottomRef.current = true;
  }, []);

  const handleSend = async () => {
    if (!inputValue.trim() || isStreaming) return;

    const content = inputValue.trim();
    setInputValue('');
    scrollToBottom(); // Scroll to bottom when user sends a message
    await sendMessage(content);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  // Handle viewing an atom from citation - switch drawer to viewer mode
  const handleViewAtom = useCallback((atomId: string) => {
    openReader(atomId);
  }, [openReader]);

  if (!currentConversation) {
    return (
      <div className="flex items-center justify-center h-full text-[var(--color-text-secondary)]">
        {isLoading ? 'Loading conversation...' : 'No conversation selected'}
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header with back button and scope */}
      <ChatHeader conversation={currentConversation} onBack={goBack} />

      {/* Messages area */}
      <div
        ref={messagesContainerRef}
        onScroll={handleScroll}
        className="flex-1 overflow-y-auto p-4 space-y-4 relative"
        style={{ overflowAnchor: 'none' }}
      >
        {/* Search bar */}
        {isSearchOpen && (
          <SearchBar
            query={searchQuery}
            searchedQuery={searchedQuery}
            onQueryChange={setSearchQuery}
            currentIndex={currentIndex}
            totalMatches={totalMatches}
            onNext={goToNext}
            onPrevious={goToPrevious}
            onClose={closeSearch}
          />
        )}
        {messages.length === 0 && !isStreaming && (
          <div className="flex flex-col items-center justify-center h-full gap-4 text-center">
            <div className="w-16 h-16 rounded-full bg-[var(--color-bg-card)] flex items-center justify-center">
              <MessageSquare className="w-8 h-8 text-[var(--color-accent)]" strokeWidth={2} />
            </div>
            <div>
              <p className="text-[var(--color-text-primary)] font-medium mb-1">Start the conversation</p>
              <p className="text-[var(--color-text-secondary)] text-sm max-w-sm">
                Ask questions about your knowledge base. The AI will search through your atoms to find relevant information.
              </p>
            </div>
          </div>
        )}

        {messages.map((message) => (
          <ChatMessage
            key={message.id}
            message={message}
            onViewAtom={handleViewAtom}
            searchQuery={isSearchOpen ? searchQuery : ''}
            highlightText={highlightText}
          />
        ))}

        {/* Thinking indicator — before any streaming content arrives */}
        {isStreaming && !streamingContent && (
          <div className="flex justify-start">
            <div className="max-w-[85%] rounded-lg px-4 py-3 bg-[var(--color-bg-card)] text-[var(--color-text-primary)]">
              <div className="flex items-center gap-2 text-sm text-[var(--color-text-secondary)]">
                <div className="flex gap-1">
                  <span className="w-1.5 h-1.5 rounded-full bg-[var(--color-accent)] animate-bounce" style={{ animationDelay: '0ms' }} />
                  <span className="w-1.5 h-1.5 rounded-full bg-[var(--color-accent)] animate-bounce" style={{ animationDelay: '150ms' }} />
                  <span className="w-1.5 h-1.5 rounded-full bg-[var(--color-accent)] animate-bounce" style={{ animationDelay: '300ms' }} />
                </div>
                <span>Thinking…</span>
              </div>
              {retrievalSteps.length > 0 && (
                <div className="mt-2 space-y-1">
                  {retrievalSteps.map((step, i) => {
                    let displayQuery = step.query;
                    try {
                      const parsed = JSON.parse(step.query);
                      displayQuery = parsed.query || parsed.search || parsed.text || step.query;
                    } catch { /* use raw */ }
                    return (
                      <div key={i} className="flex items-center gap-2 text-xs text-[var(--color-text-tertiary)]">
                        <SearchIcon className="w-3 h-3 flex-shrink-0 animate-pulse" />
                        <span className="truncate">Searching: {displayQuery}</span>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Streaming message */}
        {isStreaming && streamingContent && (
          <ChatMessage
            message={{
              id: 'streaming',
              conversation_id: currentConversation.id,
              role: 'assistant',
              content: streamingContent,
              created_at: new Date().toISOString(),
              message_index: messages.length,
              tool_calls: [],
              citations: [],
            }}
            isStreaming
            onViewAtom={handleViewAtom}
            searchQuery={isSearchOpen ? searchQuery : ''}
            highlightText={highlightText}
          />
        )}

        {/* Error message */}
        {error && (
          <div className="p-3 bg-red-500/10 border border-red-500/30 rounded-lg text-red-400 text-sm">
            {error}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <ChatInput
        value={inputValue}
        onChange={setInputValue}
        onSend={handleSend}
        onKeyDown={handleKeyDown}
        disabled={isStreaming}
        placeholder={
          currentConversation.tags.length > 0
            ? `Ask about ${currentConversation.tags.map(t => t.name).join(', ')}...`
            : 'Ask anything about your knowledge base...'
        }
      />
    </div>
  );
}
