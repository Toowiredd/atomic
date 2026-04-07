import { useEffect, useRef } from 'react';
import { useWikiStore } from '../../stores/wiki';
import { useUIStore } from '../../stores/ui';
import { WikiArticlesList } from './WikiArticlesList';
import { WikiHeader } from './WikiHeader';
import { WikiEmptyState } from './WikiEmptyState';
import { WikiGenerating } from './WikiGenerating';
import { WikiArticleContent } from './WikiArticleContent';

export function WikiFullView() {
  const view = useWikiStore(s => s.view);
  const currentTagId = useWikiStore(s => s.currentTagId);
  const currentTagName = useWikiStore(s => s.currentTagName);
  const currentArticle = useWikiStore(s => s.currentArticle);
  const articleStatus = useWikiStore(s => s.articleStatus);
  const relatedTags = useWikiStore(s => s.relatedTags);
  const wikiLinks = useWikiStore(s => s.wikiLinks);
  const isLoading = useWikiStore(s => s.isLoading);
  const isGenerating = useWikiStore(s => s.isGenerating);
  const isUpdating = useWikiStore(s => s.isUpdating);
  const error = useWikiStore(s => s.error);
  const fetchAllArticles = useWikiStore(s => s.fetchAllArticles);
  const generateArticle = useWikiStore(s => s.generateArticle);
  const updateArticle = useWikiStore(s => s.updateArticle);
  const openArticle = useWikiStore(s => s.openArticle);
  const goBack = useWikiStore(s => s.goBack);
  const clearError = useWikiStore(s => s.clearError);

  const versions = useWikiStore(s => s.versions);
  const selectedVersion = useWikiStore(s => s.selectedVersion);
  const selectVersion = useWikiStore(s => s.selectVersion);
  const clearSelectedVersion = useWikiStore(s => s.clearSelectedVersion);

  const openDrawer = useUIStore(s => s.openDrawer);

  const initializedRef = useRef(false);

  useEffect(() => {
    if (initializedRef.current) return;
    initializedRef.current = true;
    fetchAllArticles();
  }, [fetchAllArticles]);

  const handleGenerate = () => {
    if (currentTagId && currentTagName) {
      generateArticle(currentTagId, currentTagName);
    }
  };

  const handleUpdate = () => {
    if (currentTagId && currentTagName) {
      updateArticle(currentTagId, currentTagName);
    }
  };

  const handleViewAtom = (atomId: string) => {
    openDrawer('viewer', atomId);
  };

  const renderArticleContent = () => {
    if (view === 'list' || !currentTagId) {
      return (
        <div className="flex flex-col items-center justify-center h-full text-[var(--color-text-secondary)] gap-3 p-8">
          <svg className="w-12 h-12 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
          </svg>
          <p className="text-sm">Select an article to read</p>
        </div>
      );
    }

    if (isLoading) {
      return (
        <div className="flex items-center justify-center h-full text-[var(--color-text-secondary)]">
          Loading...
        </div>
      );
    }

    if (error) {
      return (
        <div className="flex flex-col items-center justify-center h-full gap-4 p-4">
          <p className="text-red-400 text-sm">{error}</p>
          <button onClick={clearError} className="text-xs text-[var(--color-accent)] hover:underline">
            Dismiss
          </button>
        </div>
      );
    }

    if (isGenerating) {
      return <WikiGenerating tagName={currentTagName || ''} atomCount={articleStatus?.current_atom_count || 0} />;
    }

    if (!currentArticle) {
      return (
        <WikiEmptyState
          tagName={currentTagName || ''}
          atomCount={articleStatus?.current_atom_count || 0}
          onGenerate={handleGenerate}
          isGenerating={false}
        />
      );
    }

    const displayArticle = selectedVersion
      ? { content: selectedVersion.content, id: selectedVersion.id, tag_id: selectedVersion.tag_id, created_at: selectedVersion.created_at, updated_at: selectedVersion.created_at, atom_count: selectedVersion.atom_count }
      : currentArticle.article;
    const displayCitations = selectedVersion
      ? selectedVersion.citations
      : currentArticle.citations;

    return (
      <div className="h-full flex flex-col overflow-hidden">
        <WikiHeader
          tagName={currentTagName || ''}
          updatedAt={selectedVersion ? selectedVersion.created_at : currentArticle.article.updated_at}
          sourceCount={displayCitations.length}
          newAtomsAvailable={selectedVersion ? 0 : (articleStatus?.new_atoms_available || 0)}
          onUpdate={handleUpdate}
          onRegenerate={handleGenerate}
          onClose={goBack}
          isUpdating={isUpdating}
          versions={versions}
          onSelectVersion={selectVersion}
          isViewingVersion={!!selectedVersion}
          onReturnToCurrent={clearSelectedVersion}
        />
        <div className="flex-1 overflow-y-auto">
          <WikiArticleContent
            article={displayArticle}
            citations={displayCitations}
            wikiLinks={selectedVersion ? [] : wikiLinks}
            relatedTags={selectedVersion ? [] : relatedTags}
            onViewAtom={handleViewAtom}
            onNavigateToArticle={(tagId, tagName) => openArticle(tagId, tagName)}
          />
        </div>
      </div>
    );
  };

  return (
    <div className="flex h-full overflow-hidden">
      {/* Left panel: article list */}
      <div className="w-72 flex-shrink-0 border-r border-[var(--color-border)] overflow-hidden">
        <WikiArticlesList />
      </div>

      {/* Right panel: article content */}
      <div className="flex-1 overflow-hidden">
        {renderArticleContent()}
      </div>
    </div>
  );
}
