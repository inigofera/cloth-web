import { ref, computed } from 'vue';

export type TabId = 'clothes' | 'outfits' | 'insights' | 'tables';

const VALID_TABS: TabId[] = ['clothes', 'outfits', 'insights', 'tables'];

export interface RouteState {
  tab: TabId;
  params: Record<string, string>;
}

function parseHash(): RouteState {
  if (typeof window === 'undefined') {
    return { tab: 'outfits', params: {} };
  }

  const hash = window.location.hash.replace(/^#\/?/, '');
  const [path, queryString] = hash.split('?');
  const tab = VALID_TABS.includes(path as TabId) ? (path as TabId) : 'outfits';
  const params: Record<string, string> = {};

  if (queryString) {
    const searchParams = new URLSearchParams(queryString);
    searchParams.forEach((val, key) => {
      params[key] = val;
    });
  }

  return { tab, params };
}

const currentRoute = ref<RouteState>(parseHash());
let isInitialized = false;

export function useRouter() {
  function navigate(tab: TabId, params?: Record<string, string>, replace = false) {
    let hash = `#${tab}`;
    if (params && Object.keys(params).length > 0) {
      const qs = new URLSearchParams(params).toString();
      hash += `?${qs}`;
    }

    if (typeof window !== 'undefined') {
      if (replace) {
        window.history.replaceState(null, '', hash);
      } else {
        window.history.pushState(null, '', hash);
      }
    }

    currentRoute.value = { tab, params: params || {} };
  }

  function init() {
    if (isInitialized || typeof window === 'undefined') return;
    isInitialized = true;

    const syncFromHash = () => {
      currentRoute.value = parseHash();
    };

    window.addEventListener('hashchange', syncFromHash);
    window.addEventListener('popstate', syncFromHash);

    if (!window.location.hash) {
      window.history.replaceState(null, '', '#outfits');
    } else {
      syncFromHash();
    }
  }

  return {
    route: currentRoute,
    currentTab: computed<TabId>({
      get: () => currentRoute.value.tab,
      set: (tab: TabId) => navigate(tab)
    }),
    params: computed(() => currentRoute.value.params),
    navigate,
    init
  };
}
