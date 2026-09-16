/**
 * Design system entry point.
 *
 * Everything in this folder is self-contained (no imports from app code).
 * The app only imports from here, so this module can later be extracted
 * into its own package without touching the rest of the frontend.
 */
import './styles/tokens.css';
import './styles/base.css';

export { default as MdAppBar } from './components/MdAppBar.vue';
export { default as MdNavDrawer } from './components/MdNavDrawer.vue';
export { default as MdButton } from './components/MdButton.vue';
export { default as MdIconButton } from './components/MdIconButton.vue';
export { default as MdListItem } from './components/MdListItem.vue';
export { MdIcon, iconNames, type IconName } from './components/icons';
export type { DrawerItem } from './types';
