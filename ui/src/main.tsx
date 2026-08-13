/**
 * The entry point: one desk, one root, and the connection started.
 *
 * The desk is created outside React on purpose. It owns a socket and a
 * reconnect timer, and neither of those belongs to a component's lifetime —
 * `StrictMode` mounts every component twice in development, and a connection
 * that opened and closed with a component would open twice and leave one of
 * them behind.
 */

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { createDesk } from "./desk";
import "./index.css";
import { DeskProvider } from "./store/context";

const desk = createDesk();
desk.start();

const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html has no #root to render into");
}

createRoot(container).render(
  <StrictMode>
    <DeskProvider store={desk.store}>
      <App />
    </DeskProvider>
  </StrictMode>,
);
