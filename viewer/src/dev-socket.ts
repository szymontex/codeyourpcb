/**
 * Dialling the dev server's hot-reload socket.
 *
 * `npm start` runs `server.ts` beside Vite, and the page in a browser dials it
 * for hot reload, routing and the workspace file list. The desktop app shipped
 * the same bundle and dialled the same `localhost:4322` on a user's machine,
 * where nothing of ours listens. Whatever did listen there could send a
 * `reload`, which replaces the editor's text, and File > Save then writes that
 * text over the file the user opened. So the desktop app does not dial unless
 * a developer asks for it with `CYPCB_DESKTOP_DEV_SOCKET=1` at build time.
 */

/** Opens the socket, or returns null when this page must not dial it. */
export function dialDevServer(
  url: string,
  desktop: boolean,
  desktopDevSocket: boolean,
): WebSocket | null {
  if (desktop && !desktopDevSocket) return null;
  return new WebSocket(url);
}
