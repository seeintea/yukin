import React from "react";
import { createRoot } from "react-dom/client";
import { Routes } from "./routes";
import "./index.css";

const root = document.getElementById("root") as HTMLElement;

createRoot(root).render(
  <React.StrictMode>
    <Routes />
  </React.StrictMode>,
);
