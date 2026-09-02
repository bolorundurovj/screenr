/**
 * Stands in for the Tauri runtime that the webview normally injects.
 *
 * `@tauri-apps/api` reads everything off `window.__TAURI_INTERNALS__`, which
 * does not exist under jsdom, so anything touching it throws before a single
 * assertion runs. Commands reject by default: a test that needs a value should
 * inject a stubbed TauriService rather than lean on this.
 */
const internals = {
    // getCurrentWindow() reads its label from here; services branch on it to
    // tell the main window apart from the recording overlay.
    metadata: {currentWindow: {label: 'main'}},
    convertFileSrc: (path: string, protocol = 'asset') => `${protocol}://localhost/${path}`,
    invoke: () => Promise.reject('No Tauri backend in tests'),
    transformCallback: (callback?: (payload: unknown) => void) => {
        void callback;
        return 0;
    },
    unregisterCallback: () => undefined,
};

Object.defineProperty(window, '__TAURI_INTERNALS__', {
    value: internals,
    writable: true,
    configurable: true,
});
