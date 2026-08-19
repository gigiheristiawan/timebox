import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Checkpoint } from "./components/Checkpoint";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";

// Both windows load this bundle; the window's own label decides what it is.
const isCheckpoint = getCurrentWindow().label === "checkpoint";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{isCheckpoint ? <Checkpoint /> : <App />}</React.StrictMode>,
);
