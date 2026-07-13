import { BrandMark } from "./components/BrandMark";
import { MediaInspector } from "./features/downloads/MediaInspector";
import { isTauri } from "@tauri-apps/api/core";

export default function App() {
  const isMacTauri =
    isTauri() && navigator.userAgent.toLowerCase().includes("mac");

  return (
    <main className={`app-shell${isMacTauri ? " is-macos-frame" : ""}`}>
      <div className="ambient-flow" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>

      <header className="app-header" data-tauri-drag-region>
        <BrandMark />
        <p className="build-label">PREVIEW · 0.1</p>
      </header>

      <section className="workspace" aria-labelledby="workspace-title">
        <div className="intro-copy">
          <h1 id="workspace-title">
            <span className="headline-line">轻取流光</span>
            <span className="headline-line headline-accent">留住此刻</span>
          </h1>
          <p className="slogan-en">
            <span>Catch the stream,</span>
            <span className="slogan-accent">Keep the moment.</span>
          </p>
        </div>

        <MediaInspector />
      </section>
    </main>
  );
}
