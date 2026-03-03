import { create } from 'zustand';
import { getTransport } from '../lib/transport';

// Types matching the Rust structs
export interface WikiArticle {
  id: string;
  tag_id: string;
  content: string;
  created_at: string;
  updated_at: string;
  atom_count: number;
}

export interface WikiCitation {
  id: string;
  citation_index: number;
  atom_id: string;
  chunk_index: number | null;
  excerpt: string;
}

export interface WikiArticleWithCitations {
  article: WikiArticle;
  citations: WikiCitation[];
}

export interface WikiArticleStatus {
  has_article: boolean;
  article_atom_count: number;
  current_atom_count: number;
  new_atoms_available: number;
  updated_at: string | null;
}

export interface WikiArticleSummary {
  id: string;
  tag_id: string;
  tag_name: string;
  updated_at: string;
  atom_count: number;
  inbound_links: number;
}

export interface WikiLink {
  id: string;
  source_article_id: string;
  target_tag_name: string;
  target_tag_id: string | null;
  has_article: boolean;
}

export interface RelatedTag {
  tag_id: string;
  tag_name: string;
  score: number;
  shared_atoms: number;
  semantic_edges: number;
  has_article: boolean;
}

export interface SuggestedArticle {
  tag_id: string;
  tag_name: string;
  atom_count: number;
  mention_count: number;
  score: number;
}

export interface WikiVersionSummary {
  id: string;
  version_number: number;
  atom_count: number;
  created_at: string;
}

export interface WikiArticleVersion {
  id: string;
  tag_id: string;
  content: string;
  citations: WikiCitation[];
  atom_count: number;
  version_number: number;
  created_at: string;
}

type WikiView = 'list' | 'article';

interface WikiStore {
  // View state
  view: WikiView;
  currentTagId: string | null;
  currentTagName: string | null;

  // Articles list state
  articles: WikiArticleSummary[];
  isLoadingList: boolean;

  // Suggestions state
  suggestedArticles: SuggestedArticle[];
  isLoadingSuggestions: boolean;

  // Current article state
  currentArticle: WikiArticleWithCitations | null;
  articleStatus: WikiArticleStatus | null;
  relatedTags: RelatedTag[];
  wikiLinks: WikiLink[];

  // Version history
  versions: WikiVersionSummary[];
  selectedVersion: WikiArticleVersion | null;

  // Loading states
  isLoading: boolean;
  isGenerating: boolean;
  isUpdating: boolean;
  error: string | null;

  // List actions
  fetchAllArticles: () => Promise<void>;
  fetchSuggestedArticles: () => Promise<void>;
  showList: () => void;
  openArticle: (tagId: string, tagName: string) => void;
  openAndGenerate: (tagId: string, tagName: string) => void;
  goBack: () => void;

  // Article actions
  fetchArticle: (tagId: string) => Promise<void>;
  fetchArticleStatus: (tagId: string) => Promise<void>;
  fetchRelatedTags: (tagId: string) => Promise<void>;
  fetchWikiLinks: (tagId: string) => Promise<void>;
  generateArticle: (tagId: string, tagName: string) => Promise<void>;
  updateArticle: (tagId: string, tagName: string) => Promise<void>;
  deleteArticle: (tagId: string) => Promise<void>;
  fetchVersions: (tagId: string) => Promise<void>;
  selectVersion: (versionId: string) => Promise<void>;
  clearSelectedVersion: () => void;
  clearArticle: () => void;
  clearError: () => void;
  reset: () => void;
}

export const useWikiStore = create<WikiStore>((set, get) => ({
  // View state
  view: 'list',
  currentTagId: null,
  currentTagName: null,

  // Articles list state
  articles: [],
  isLoadingList: false,

  // Suggestions state
  suggestedArticles: [],
  isLoadingSuggestions: false,

  // Current article state
  currentArticle: null,
  articleStatus: null,
  relatedTags: [],
  wikiLinks: [],
  versions: [],
  selectedVersion: null,
  isLoading: false,
  isGenerating: false,
  isUpdating: false,
  error: null,

  fetchAllArticles: async () => {
    set({ isLoadingList: true, error: null });
    try {
      const articles = await getTransport().invoke<WikiArticleSummary[]>('get_all_wiki_articles');
      set({ articles, isLoadingList: false });
      // Refresh suggestions after a brief yield so the list renders first
      setTimeout(() => get().fetchSuggestedArticles(), 50);
    } catch (error) {
      set({ error: String(error), isLoadingList: false });
    }
  },

  fetchSuggestedArticles: async () => {
    set({ isLoadingSuggestions: true });
    try {
      const suggestions = await getTransport().invoke<SuggestedArticle[]>('get_suggested_wiki_articles', { limit: 100 });
      set({ suggestedArticles: suggestions, isLoadingSuggestions: false });
    } catch (error) {
      console.error('Failed to fetch suggested articles:', error);
      set({ isLoadingSuggestions: false });
    }
  },

  showList: () => {
    set({
      view: 'list',
      currentTagId: null,
      currentTagName: null,
      currentArticle: null,
      articleStatus: null,
      relatedTags: [],
      wikiLinks: [],
      versions: [],
      selectedVersion: null,
      error: null,
    });
  },

  openArticle: (tagId: string, tagName: string) => {
    set({
      view: 'article',
      currentTagId: tagId,
      currentTagName: tagName,
      currentArticle: null,
      articleStatus: null,
      relatedTags: [],
      wikiLinks: [],
      versions: [],
      selectedVersion: null,
      isLoading: true,
      error: null,
    });
    // Fetch article, status, related tags, wiki links, and versions
    get().fetchArticle(tagId);
    get().fetchArticleStatus(tagId);
    get().fetchRelatedTags(tagId);
    get().fetchWikiLinks(tagId);
    get().fetchVersions(tagId);
  },

  // Open article view and immediately start generating (for new wikis)
  openAndGenerate: (tagId: string, tagName: string) => {
    set({
      view: 'article',
      currentTagId: tagId,
      currentTagName: tagName,
      currentArticle: null,
      articleStatus: null,
      relatedTags: [],
      wikiLinks: [],
      versions: [],
      selectedVersion: null,
      isLoading: false,
      isGenerating: true,
      error: null,
    });
    // Fetch status for display during generation
    get().fetchArticleStatus(tagId);
    // Start generation
    get().generateArticle(tagId, tagName);
  },

  goBack: () => {
    set({
      view: 'list',
      currentTagId: null,
      currentTagName: null,
      currentArticle: null,
      articleStatus: null,
      relatedTags: [],
      wikiLinks: [],
      versions: [],
      selectedVersion: null,
      error: null,
    });
    // Refresh list in case changes were made
    get().fetchAllArticles();
  },

  fetchArticle: async (tagId: string) => {
    set({ isLoading: true, error: null });
    try {
      const article = await getTransport().invoke<WikiArticleWithCitations | null>('get_wiki_article', { tagId });
      set({ currentArticle: article, isLoading: false });
    } catch (error) {
      set({ error: String(error), isLoading: false });
    }
  },

  fetchArticleStatus: async (tagId: string) => {
    try {
      const status = await getTransport().invoke<WikiArticleStatus>('get_wiki_article_status', { tagId });
      set({ articleStatus: status });
    } catch (error) {
      console.error('Failed to fetch article status:', error);
    }
  },

  fetchRelatedTags: async (tagId: string) => {
    try {
      const tags = await getTransport().invoke<RelatedTag[]>('get_related_tags', { tagId, limit: 10 });
      set({ relatedTags: tags });
    } catch (error) {
      console.error('Failed to fetch related tags:', error);
    }
  },

  fetchWikiLinks: async (tagId: string) => {
    try {
      const links = await getTransport().invoke<WikiLink[]>('get_wiki_links', { tagId });
      set({ wikiLinks: links });
    } catch (error) {
      console.error('Failed to fetch wiki links:', error);
    }
  },

  generateArticle: async (tagId: string, tagName: string) => {
    set({ isGenerating: true, error: null, selectedVersion: null });
    try {
      const article = await getTransport().invoke<WikiArticleWithCitations>('generate_wiki_article', { tagId, tagName });
      set({ currentArticle: article, isGenerating: false });
      // Refresh status, related tags, wiki links, and versions after generation
      get().fetchArticleStatus(tagId);
      get().fetchRelatedTags(tagId);
      get().fetchWikiLinks(tagId);
      get().fetchVersions(tagId);
      // Also refresh the list to include the new article
      get().fetchAllArticles();
    } catch (error) {
      set({ error: String(error), isGenerating: false });
    }
  },

  updateArticle: async (tagId: string, tagName: string) => {
    set({ isUpdating: true, error: null, selectedVersion: null });
    try {
      const article = await getTransport().invoke<WikiArticleWithCitations>('update_wiki_article', { tagId, tagName });
      set({ currentArticle: article, isUpdating: false });
      // Refresh status, related tags, wiki links, and versions after update
      get().fetchArticleStatus(tagId);
      get().fetchRelatedTags(tagId);
      get().fetchWikiLinks(tagId);
      get().fetchVersions(tagId);
      // Also refresh the list
      get().fetchAllArticles();
    } catch (error) {
      set({ error: String(error), isUpdating: false });
    }
  },

  deleteArticle: async (tagId: string) => {
    try {
      await getTransport().invoke('delete_wiki_article', { tagId });
      set({ currentArticle: null, articleStatus: null, relatedTags: [], wikiLinks: [], versions: [], selectedVersion: null });
      // Refresh the list
      get().fetchAllArticles();
    } catch (error) {
      set({ error: String(error) });
    }
  },

  fetchVersions: async (tagId: string) => {
    try {
      const versions = await getTransport().invoke<WikiVersionSummary[]>('get_wiki_versions', { tagId });
      set({ versions });
    } catch (error) {
      console.error('Failed to fetch wiki versions:', error);
    }
  },

  selectVersion: async (versionId: string) => {
    try {
      const version = await getTransport().invoke<WikiArticleVersion | null>('get_wiki_version', { versionId });
      set({ selectedVersion: version });
    } catch (error) {
      console.error('Failed to fetch wiki version:', error);
    }
  },

  clearSelectedVersion: () => {
    set({ selectedVersion: null });
  },

  clearArticle: () => {
    set({ currentArticle: null, articleStatus: null, relatedTags: [], wikiLinks: [], versions: [], selectedVersion: null, error: null });
  },

  clearError: () => {
    set({ error: null });
  },

  reset: () => {
    set({
      view: 'list',
      currentTagId: null,
      currentTagName: null,
      articles: [],
      isLoadingList: false,
      suggestedArticles: [],
      isLoadingSuggestions: false,
      currentArticle: null,
      articleStatus: null,
      relatedTags: [],
      wikiLinks: [],
      versions: [],
      selectedVersion: null,
      isLoading: false,
      isGenerating: false,
      isUpdating: false,
      error: null,
    });
  },
}));
