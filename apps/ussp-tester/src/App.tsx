import { useState } from "react";
import Sender from "./components/Sender";
import Receiver from "./components/Receiver";

type Mode = "sender" | "receiver";

function App() {
  const [mode, setMode] = useState<Mode>("sender");

  return (
    <div className="app">
      <header className="header">
        <h1>USSP Tester</h1>
        <div className="mode-toggle">
          <button
            className={mode === "sender" ? "active" : ""}
            onClick={() => setMode("sender")}
          >
            Sender
          </button>
          <button
            className={mode === "receiver" ? "active" : ""}
            onClick={() => setMode("receiver")}
          >
            Receiver
          </button>
        </div>
      </header>

      <main className="main-content">
        {mode === "sender" ? <Sender /> : <Receiver />}
      </main>
    </div>
  );
}

export default App;
