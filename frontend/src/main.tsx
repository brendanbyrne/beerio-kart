import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './index.css';
import { App } from './App';

const rootEl = document.getElementById('root');
if (!rootEl) throw new Error('Root element missing');
createRoot(rootEl).render(
  <StrictMode>
    <App />
  </StrictMode>,
);

// Dev-only runtime accessibility auditing (react.md § 10). @axe-core/react
// re-runs axe-core after each render and logs violations the static linter
// can't see — focus traps, color contrast, live regions — to the browser
// console. Dynamically imported behind the DEV guard so axe-core never ships
// in the production bundle.
if (import.meta.env.DEV) {
  void import('@axe-core/react').then(async ({ default: reactAxe }) => {
    const React = await import('react');
    const ReactDOM = await import('react-dom');
    await reactAxe(React, ReactDOM, 1000);
  });
}
