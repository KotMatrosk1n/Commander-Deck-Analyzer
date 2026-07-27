import React from "react";
import ReactDOM from "react-dom/client";
import { RotateCcw, TriangleAlert } from "lucide-react";
import App from "./App";
import { UiErrorBoundary } from "./UiErrorBoundary";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <UiErrorBoundary
      fallback={({ error, reset }) => (
        <main
          className="fatal-recovery"
          aria-labelledby="fatal-recovery-title"
          role="alert"
        >
          <section className="recovery-card">
            <div className="recovery-icon"><TriangleAlert size={24} /></div>
            <span className="eyebrow">Interface recovery</span>
            <h1 id="fatal-recovery-title">The interface hit an unexpected error</h1>
            <p>
              The analyzer stopped this view before it could leave you on a blank
              screen. Restart the interface to return to the deck workspace.
            </p>
            <button className="recovery-primary" onClick={reset} type="button">
              <RotateCcw size={16} />
              Restart interface
            </button>
            <details>
              <summary>Technical details</summary>
              <code>{error.message || "Unknown rendering error"}</code>
            </details>
          </section>
        </main>
      )}
    >
      <App />
    </UiErrorBoundary>
  </React.StrictMode>,
);
