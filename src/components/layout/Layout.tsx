import { useEffect } from 'react';
import { LeftPanel } from './LeftPanel';
import { MainView } from './MainView';
import { RightDrawer } from './RightDrawer';
import { useAtomsStore } from '../../stores/atoms';
import { useTagsStore } from '../../stores/tags';

export function Layout() {
  const { fetchAtoms } = useAtomsStore();
  const { fetchTags } = useTagsStore();

  // Fetch initial data
  useEffect(() => {
    fetchAtoms();
    fetchTags();
  }, [fetchAtoms, fetchTags]);

  return (
    <div className="flex h-screen overflow-hidden bg-[#1e1e1e]">
      <LeftPanel />
      <MainView />
      <RightDrawer />
    </div>
  );
}

