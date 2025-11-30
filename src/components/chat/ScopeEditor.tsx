import { useState } from 'react';
import { ConversationWithTags, useChatStore } from '../../stores/chat';
import { useTagsStore } from '../../stores/tags';

interface ScopeEditorProps {
  conversation: ConversationWithTags;
}

export function ScopeEditor({ conversation }: ScopeEditorProps) {
  const [isAdding, setIsAdding] = useState(false);
  const { addTagToScope, removeTagFromScope } = useChatStore();
  const { tags: allTags } = useTagsStore();

  // Flatten tag tree for selection
  const flattenTags = (tags: typeof allTags): { id: string; name: string; depth: number }[] => {
    const result: { id: string; name: string; depth: number }[] = [];
    const traverse = (tags: typeof allTags, depth: number) => {
      for (const tag of tags) {
        result.push({ id: tag.id, name: tag.name, depth });
        if (tag.children?.length > 0) {
          traverse(tag.children, depth + 1);
        }
      }
    };
    traverse(tags, 0);
    return result;
  };

  const availableTags = flattenTags(allTags).filter(
    (tag) => !conversation.tags.some((t) => t.id === tag.id)
  );

  const handleAddTag = async (tagId: string) => {
    await addTagToScope(tagId);
    setIsAdding(false);
  };

  const handleRemoveTag = async (tagId: string) => {
    await removeTagFromScope(tagId);
  };

  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-xs text-[#666666] uppercase tracking-wide">Scope:</span>

      {conversation.tags.length === 0 ? (
        <span className="text-sm text-[#888888] italic">All atoms</span>
      ) : (
        conversation.tags.map((tag) => (
          <span
            key={tag.id}
            className="group inline-flex items-center gap-1 px-2 py-0.5 text-sm rounded bg-[#7c3aed]/20 text-[#a78bfa]"
          >
            {tag.name}
            <button
              onClick={() => handleRemoveTag(tag.id)}
              className="opacity-0 group-hover:opacity-100 hover:text-red-400 transition-all"
              aria-label={`Remove ${tag.name} from scope`}
            >
              <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
              </svg>
            </button>
          </span>
        ))
      )}

      {/* Add tag button/dropdown */}
      {isAdding ? (
        <div className="relative">
          <select
            autoFocus
            onChange={(e) => {
              if (e.target.value) {
                handleAddTag(e.target.value);
              }
            }}
            onBlur={() => setIsAdding(false)}
            className="appearance-none bg-[#1e1e1e] border border-[#3d3d3d] rounded px-2 py-1 text-sm text-[#dcddde] focus:outline-none focus:border-[#7c3aed] cursor-pointer"
          >
            <option value="">Select tag...</option>
            {availableTags.map((tag) => (
              <option key={tag.id} value={tag.id}>
                {'  '.repeat(tag.depth)}{tag.name}
              </option>
            ))}
          </select>
        </div>
      ) : (
        <button
          onClick={() => setIsAdding(true)}
          className="inline-flex items-center gap-1 px-2 py-0.5 text-sm rounded border border-dashed border-[#3d3d3d] text-[#888888] hover:border-[#7c3aed] hover:text-[#a78bfa] transition-colors"
        >
          <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
          </svg>
          Add tag
        </button>
      )}
    </div>
  );
}
