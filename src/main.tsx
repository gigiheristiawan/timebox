import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { Checkpoint } from "./components/Checkpoint";
import { Popover } from "./components/Popover";
import { Settings } from "./components/Settings";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./styles.css";

// Every window loads this bundle; the window's own label decides what it is.
const label = getCurrentWindow().label;
document.body.dataset.surface = label;

const surface = {
  checkpoint: <Checkpoint />,
  popover: <Popover />,
  settings: <Settings />,
}[label] ?? <App />;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{surface}</React.StrictMode>,
);
