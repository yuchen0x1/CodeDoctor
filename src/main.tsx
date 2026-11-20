import React from "react";
import ReactDOM from "react-dom/client";
import App from "./app/App";
import { readSettings } from "./lib/settings";
import "./styles/app.css";

document.documentElement.dataset.theme = readSettings().theme;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
