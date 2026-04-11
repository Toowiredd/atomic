import { useEffect } from 'react';
import { dashboardWidgets } from './registry';
import { useWikiStore } from '../../stores/wiki';

export function DashboardView() {
  const fetchAllArticles = useWikiStore(s => s.fetchAllArticles);

  useEffect(() => {
    // Kick off wiki data on dashboard mount. The call is idempotent — safe to
    // fire every time the user lands on the dashboard so widgets stay fresh.
    fetchAllArticles();
  }, [fetchAllArticles]);

  return (
    <div className="h-full overflow-y-auto">
      <div className="mx-auto max-w-4xl px-6 pt-10 pb-16 md:px-10 md:pt-14 md:pb-20">
        <div className="grid grid-cols-1 md:grid-cols-2 gap-x-10 gap-y-10 md:gap-y-12">
          {dashboardWidgets.map(({ id, span, Component }) => (
            <div key={id} className={span === 'full' ? 'md:col-span-2' : ''}>
              <Component />
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
