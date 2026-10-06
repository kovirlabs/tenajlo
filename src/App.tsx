import { StartupGate } from "./components/StartupGate";
import { AppShell } from "./views/AppShell";

export function App() {
  return (
    <StartupGate>
      <AppShell />
    </StartupGate>
  );
}
